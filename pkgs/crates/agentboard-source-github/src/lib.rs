use std::{collections::HashSet, io, process::Command};

#[cfg(test)]
use std::sync::Arc;

use agentboard_core::{
    model::Item,
    registry::{
        RuntimeResult, Source, SourceCollection, SourceContext, SourceDefinition, SourceFuture,
    },
};
use anyhow::{anyhow, bail, Context, Result};
use command_group::AsyncCommandGroup;
use reqwest::{header, Client};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const GITHUB_SEARCH_URL: &str = "https://api.github.com/search/issues";

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GithubSourceConfig {
    pub mode: GithubSourceMode,
    pub query: String,
    pub credentials: GithubCredentialConfig,
    #[serde(default = "default_source_limit")]
    pub limit: usize,
    #[serde(default)]
    pub field_map: agentboard_core::model::FieldMap,
    pub status_map: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum GithubSourceMode {
    Issue,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GithubCredentialConfig {
    pub helper: String,
}

pub struct GithubSourceDefinition;

pub struct GithubSource {
    config: GithubSourceConfig,
    search_url: String,
    #[cfg(test)]
    client_completed: Option<Arc<std::sync::atomic::AtomicUsize>>,
}

impl SourceDefinition for GithubSourceDefinition {
    const ID: &'static str = "github";
    type Config = GithubSourceConfig;
    type Runtime = GithubSource;

    fn build(config: Self::Config) -> RuntimeResult<Self::Runtime> {
        if config.query.trim().is_empty() {
            bail!("requires query");
        }
        if config.credentials.helper.trim().is_empty() {
            bail!("credential helper cannot be empty");
        }
        if config.status_map.is_empty() {
            bail!("requires status_map");
        }
        if config
            .status_map
            .iter()
            .any(|(label, status)| label.trim().is_empty() || status.trim().is_empty())
        {
            bail!("status_map cannot contain empty labels or statuses");
        }
        if config.limit == 0 {
            bail!("limit must be greater than zero");
        }
        Ok(GithubSource {
            config,
            search_url: GITHUB_SEARCH_URL.into(),
            #[cfg(test)]
            client_completed: None,
        })
    }
}

impl GithubSource {
    async fn collect_github_issues(
        &self,
        source_id: &str,
        cancellation: &agentboard_core::CancellationToken,
    ) -> Result<SourceCollection> {
        let token = github_token(&self.config.credentials, cancellation).await?;
        stop_if_cancelled(cancellation, "github collection")?;
        let client = Client::new();
        let search_query = issue_only_query(&self.config.query);
        eprintln!("github source {source_id} query: {search_query}");
        let mut page = 1usize;
        let mut items = Vec::new();
        let mut available = None;

        while items.len() < self.config.limit {
            stop_if_cancelled(cancellation, "github pagination")?;
            let page_size = (self.config.limit - items.len()).min(100);
            let response = github_issue_search(
                &client,
                &self.search_url,
                &token,
                &search_query,
                page_size,
                page,
                cancellation,
            )
            .await?;

            stop_if_cancelled(cancellation, "github pagination")?;
            let total = response
                .get("total_count")
                .and_then(Value::as_u64)
                .ok_or_else(|| anyhow!("github issue search response missing total_count"))?
                as usize;
            available.get_or_insert(total);
            let issues = response
                .get("items")
                .and_then(Value::as_array)
                .ok_or_else(|| anyhow!("github issue search response missing items array"))?;
            if issues.is_empty() {
                break;
            }

            for issue in issues {
                let item = normalize_issue_cooperatively(
                    source_id,
                    issue,
                    &self.config.field_map,
                    &self.config.status_map,
                    cancellation,
                )
                .await?;

                items.push(item);
                if items.len() >= self.config.limit {
                    break;
                }
            }
            page += 1;
        }

        let collection = self.collection_from_items(source_id, items, available.unwrap_or(0))?;
        #[cfg(test)]
        if let Some(completed) = &self.client_completed {
            completed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
        Ok(collection)
    }

    fn collection_from_items(
        &self,
        source_id: &str,
        items: Vec<Item>,
        available: usize,
    ) -> Result<SourceCollection> {
        let mut ids = HashSet::new();
        for item in &items {
            if !ids.insert(&item.id) {
                bail!("duplicate item id {} in source {source_id}", item.id);
            }
        }
        Ok(SourceCollection {
            items,
            available: Some(available),
            limit: self.config.limit,
        })
    }
}

impl Source for GithubSource {
    fn collect<'a>(&'a self, context: &'a SourceContext<'a>) -> SourceFuture<'a> {
        Box::pin(async move {
            match self.config.mode {
                GithubSourceMode::Issue => {
                    self.collect_github_issues(context.source_id, &context.cancellation)
                        .await
                }
            }
        })
    }

    fn item_bucket_identity(&self) -> String {
        "github.com".into()
    }
}

fn default_source_limit() -> usize {
    50
}

async fn github_issue_search(
    client: &Client,
    url: &str,
    token: &str,
    query: &str,
    per_page: usize,
    page: usize,
    cancellation: &agentboard_core::CancellationToken,
) -> Result<Value> {
    stop_if_cancelled(cancellation, "github issue search")?;
    let request = client
        .get(url)
        .bearer_auth(token)
        .header(header::ACCEPT, "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header(header::USER_AGENT, "agentboard")
        .query(&[
            ("q", query),
            ("per_page", &per_page.to_string()),
            ("page", &page.to_string()),
        ])
        .send();
    let response = tokio::select! {
        biased;
        response = request => response.context("send github issue search request")?,
        _ = cancellation.cancelled() => bail!("github issue search cancelled"),
    };
    let status = response.status();
    let text = tokio::select! {
        biased;
        text = response.text() => text.context("read github issue search response")?,
        _ = cancellation.cancelled() => bail!("github issue search cancelled"),
    };
    if !status.is_success() {
        bail!("github issue search failed with {status}: {text}");
    }
    parse_github_response(text, cancellation).await
}

async fn parse_github_response(
    text: String,
    cancellation: &agentboard_core::CancellationToken,
) -> Result<Value> {
    let task_cancellation = cancellation.clone();
    let mut task = tokio::task::spawn_blocking(move || {
        let reader = CancellationReader {
            bytes: text.into_bytes(),
            offset: 0,
            cancellation: task_cancellation,
        };
        serde_json::from_reader(reader).context("parse github issue search JSON")
    });

    let result = tokio::select! {
        biased;
        result = &mut task => result.context("parse github issue search task")?,
        _ = cancellation.cancelled() => {
            let _ = task.await;
            bail!("github issue search cancelled");
        }
    };
    let result = match result {
        Ok(result) => result,
        Err(_error) if cancellation.is_cancelled() => bail!("github issue search cancelled"),
        Err(error) => return Err(error),
    };
    stop_if_cancelled(cancellation, "github issue search")?;
    Ok(result)
}

struct CancellationReader {
    bytes: Vec<u8>,
    offset: usize,
    cancellation: agentboard_core::CancellationToken,
}

impl io::Read for CancellationReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.cancellation.is_cancelled() {
            return Err(io::Error::other("github issue search cancelled"));
        }
        if self.offset == self.bytes.len() {
            return Ok(0);
        }
        let length = buffer.len().min(1024).min(self.bytes.len() - self.offset);
        buffer[..length].copy_from_slice(&self.bytes[self.offset..self.offset + length]);
        self.offset += length;
        Ok(length)
    }
}

fn stop_if_cancelled(cancellation: &agentboard_core::CancellationToken, stage: &str) -> Result<()> {
    if cancellation.is_cancelled() {
        bail!("{stage} cancelled");
    }
    Ok(())
}

#[cfg(test)]
fn normalize_issue(
    source_id: &str,
    issue: &Value,
    field_map: &agentboard_core::model::FieldMap,
    status_map: &std::collections::BTreeMap<String, String>,
) -> Result<Item> {
    normalize_issue_with_check(source_id, issue, field_map, status_map, || Ok(()))
}

fn normalize_issue_with_check<F>(
    source_id: &str,
    issue: &Value,
    field_map: &agentboard_core::model::FieldMap,
    status_map: &std::collections::BTreeMap<String, String>,
    mut check: F,
) -> Result<Item>
where
    F: FnMut() -> Result<()>,
{
    check()?;
    if issue.get("pull_request").is_some() {
        bail!("github issue search returned pull request; query must exclude pull requests");
    }
    check()?;

    let repo_url = string_field(
        issue.pointer("/repository_url"),
        "github issue repository_url",
    )?;
    check()?;
    let repo = repo_url
        .strip_prefix("https://api.github.com/repos/")
        .ok_or_else(|| anyhow!("github issue repository_url has unexpected format"))?;
    check()?;
    let number = issue
        .get("number")
        .and_then(Value::as_i64)
        .ok_or_else(|| anyhow!("github issue number must be an integer"))?;
    check()?;
    let id = format!("{repo}#{number}");
    let reference_id = match field_map.id.as_deref() {
        Some(path) => mapped_field(issue, path, "id")?,
        None => number.to_string(),
    };
    check()?;
    let title = mapped_field(
        issue,
        field_map.title.as_deref().unwrap_or("title"),
        "title",
    )?;
    check()?;
    let state = mapped_field(
        issue,
        field_map.status.as_deref().unwrap_or("state"),
        "status",
    )?;
    check()?;
    let url = mapped_field(issue, field_map.url.as_deref().unwrap_or("html_url"), "url")?;
    check()?;
    let status = mapped_status(issue, status_map)
        .unwrap_or_else(|| status_map.get(&state).cloned().unwrap_or(state));
    check()?;

    Ok(Item {
        id,
        reference_id,
        title,
        status,
        url,
        source_id: source_id.to_string(),
        source_kind: "github".to_string(),
        raw: json!({ "github": { "issue": issue } }),
    })
}

async fn normalize_issue_cooperatively(
    source_id: &str,
    issue: &Value,
    field_map: &agentboard_core::model::FieldMap,
    status_map: &std::collections::BTreeMap<String, String>,
    cancellation: &agentboard_core::CancellationToken,
) -> Result<Item> {
    let source_id = source_id.to_string();
    let issue = issue.clone();
    let field_map = field_map.clone();
    let status_map = status_map.clone();
    let task_cancellation = cancellation.clone();
    let mut task = tokio::task::spawn_blocking(move || {
        normalize_issue_with_check(&source_id, &issue, &field_map, &status_map, || {
            stop_if_cancelled(&task_cancellation, "github normalization")
        })
    });

    tokio::select! {
        biased;
        result = &mut task => result.context("normalize github issue task")?,
        _ = cancellation.cancelled() => {
            let _ = task.await;
            bail!("github normalization cancelled");
        }
    }
}

fn mapped_status(
    issue: &Value,
    status_map: &std::collections::BTreeMap<String, String>,
) -> Option<String> {
    let labels = issue.get("labels")?.as_array()?;
    for label in labels {
        let Some(name) = label.get("name").and_then(Value::as_str) else {
            continue;
        };
        if let Some(status) = status_map.get(name) {
            return Some(status.clone());
        }
    }
    None
}

fn mapped_field(value: &Value, path: &str, name: &str) -> Result<String> {
    let mut current = value;
    for part in path.split('.') {
        current = current
            .get(part)
            .ok_or_else(|| anyhow!("github field_map {name}={path} must resolve to a string"))?;
    }
    current
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("github field_map {name}={path} must resolve to a string"))
}

fn string_field(value: Option<&Value>, name: &str) -> Result<String> {
    value
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| anyhow!("{name} must be a string"))
}

fn issue_only_query(query: &str) -> String {
    if query.split_whitespace().any(|part| part == "is:issue") {
        query.to_string()
    } else {
        format!("is:issue {query}")
    }
}

async fn github_token(
    credentials: &GithubCredentialConfig,
    cancellation: &agentboard_core::CancellationToken,
) -> Result<String> {
    if credentials.helper.trim().is_empty() {
        bail!("github credential helper cannot be empty");
    }
    stop_if_cancelled(cancellation, "github credential helper")?;
    let mut child = tokio::process::Command::from(shell_command(&credentials.helper))
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .group_spawn()
        .with_context(|| format!("run github credential helper {}", credentials.helper))?;
    let stdout = child
        .inner()
        .stdout
        .take()
        .ok_or_else(|| anyhow!("github helper stdout was not captured"))?;
    let stderr = child
        .inner()
        .stderr
        .take()
        .ok_or_else(|| anyhow!("github helper stderr was not captured"))?;
    let output = tokio::select! {
        biased;
        result = async {
            use tokio::io::AsyncReadExt;
            let mut stdout = stdout;
            let mut stderr = stderr;
            let mut out = Vec::new();
            let mut err = Vec::new();
            let (status, out_result, err_result) = tokio::join!(child.wait(), stdout.read_to_end(&mut out), stderr.read_to_end(&mut err));
            out_result?;
            err_result?;
            Ok::<_, anyhow::Error>(std::process::Output { status: status?, stdout: out, stderr: err })
        } => result?,
        _ = cancellation.cancelled() => {
            let _ = child.start_kill();
            let _ = child.wait().await;
            bail!("github credential helper cancelled");
        }
    };
    if !output.status.success() {
        bail!(
            "github credential helper failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if token.is_empty() {
        bail!("github credential helper returned empty token");
    }
    Ok(token)
}

fn shell_command(command: &str) -> Command {
    #[cfg(windows)]
    {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", command]);
        cmd
    }
    #[cfg(not(windows))]
    {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", command]);
        cmd
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentboard_core::{
        registry::{HealthCheckContext, RawConfig, Registry, Source, SourceDefinition},
        CancellationToken,
    };
    use std::{
        collections::BTreeMap,
        io::{Read, Write},
        net::{Shutdown, TcpListener},
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
        thread,
        time::{Duration, Instant},
    };

    struct MockResponse {
        body: String,
        delay_before_accept: Duration,
        delay_before_response: Duration,
    }

    fn mock_server(responses: Vec<MockResponse>) -> (String, Arc<AtomicUsize>, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(AtomicUsize::new(0));
        let completed = Arc::new(AtomicUsize::new(0));
        let request_count = Arc::clone(&requests);
        let completed_count = Arc::clone(&completed);
        thread::spawn(move || {
            for response in responses {
                thread::sleep(response.delay_before_accept);
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                request_count.fetch_add(1, Ordering::SeqCst);
                let mut request = Vec::new();
                let mut byte = [0_u8; 1];
                while !request.ends_with(b"\r\n\r\n") {
                    if stream.read(&mut byte).unwrap_or(0) == 0 {
                        break;
                    }
                    request.push(byte[0]);
                }
                thread::sleep(response.delay_before_response);
                let message = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    response.body.len(),
                    response.body
                );
                let _ = stream.write_all(message.as_bytes());
                let _ = stream.shutdown(Shutdown::Both);
                completed_count.fetch_add(1, Ordering::SeqCst);
            }
        });
        (
            format!("http://{address}/search/issues"),
            requests,
            completed,
        )
    }

    async fn wait_for_count(counter: &AtomicUsize, expected: usize) {
        let deadline = Instant::now() + Duration::from_secs(1);
        while counter.load(Ordering::SeqCst) < expected {
            assert!(
                Instant::now() < deadline,
                "timed out waiting for request count"
            );
            tokio::task::yield_now().await;
        }
    }

    fn response_with_issues(issues: Vec<Value>) -> MockResponse {
        MockResponse {
            body: json!({"total_count": issues.len(), "items": issues}).to_string(),
            delay_before_accept: Duration::ZERO,
            delay_before_response: Duration::ZERO,
        }
    }

    fn issue(number: i64) -> Value {
        json!({
            "repository_url": "https://api.github.com/repos/zenobi-us/agentboard",
            "number": number,
            "title": format!("Issue {number}"),
            "state": "open",
            "html_url": format!("https://github.com/zenobi-us/agentboard/issues/{number}"),
            "labels": []
        })
    }

    fn source_with_url(url: String) -> GithubSource {
        let mut config = config();
        config.credentials.helper = "printf token".into();
        GithubSource {
            config,
            search_url: url,
            client_completed: None,
        }
    }

    fn config() -> GithubSourceConfig {
        serde_json::from_value(json!({
            "mode": "issue",
            "query": "repo:zenobi-us/agentboard is:open",
            "credentials": {"helper": "exit 7"},
            "status_map": {"ready": "ready"}
        }))
        .unwrap()
    }

    #[test]
    fn registers_github_config_schema() {
        let mut registry = Registry::new();
        registry.add_source::<GithubSourceDefinition>().unwrap();

        let registration = registry.sources().next().unwrap();
        let schema = serde_json::to_value(registration.schema()).unwrap();

        assert_eq!(registration.id(), "github");
        assert!(schema["properties"]["mode"].is_object());
        assert!(schema["properties"]["query"].is_object());
        assert_eq!(
            schema["properties"]["mode"]["$ref"],
            "#/definitions/GithubSourceMode"
        );
        assert_eq!(schema["properties"]["limit"]["default"], 50);
        assert_eq!(schema["additionalProperties"], false);
        let required = schema["required"].as_array().unwrap();
        for field in ["mode", "query", "credentials", "status_map"] {
            assert!(required.iter().any(|value| value == field));
        }
        assert_eq!(
            schema["definitions"]["GithubSourceMode"]["enum"],
            json!(["issue"])
        );
        assert_eq!(config().limit, 50);

        let source = registry
            .build_source(
                "github",
                serde_json::from_value::<RawConfig>(json!({
                    "mode": "issue",
                    "query": "repo:zenobi-us/agentboard is:open",
                    "credentials": {"helper": "exit 7"},
                    "status_map": {"ready": "ready"}
                }))
                .unwrap(),
            )
            .unwrap();
        assert_eq!(source.item_bucket_identity(), "github.com");
        assert!(registry
            .build_source(
                "github",
                serde_json::from_value::<RawConfig>(json!({
                    "mode": "issue",
                    "query": "repo:zenobi-us/agentboard is:open",
                    "credentials": {"helper": "exit 7"},
                    "status_map": {"ready": "ready"},
                    "extra": true
                }))
                .unwrap(),
            )
            .is_err());
    }

    #[test]
    fn deserializes_existing_github_toml_fields() {
        let config: GithubSourceConfig = toml::from_str(
            r#"
                mode = "issue"
                query = "repo:zenobi-us/agentboard is:open label:ready"
                credentials = { helper = "gh auth token" }
                status_map = { ready = "ready" }
            "#,
        )
        .unwrap();

        assert!(GithubSourceDefinition::build(config).is_ok());
    }

    #[test]
    fn validates_github_config_without_running_credential_helper() {
        assert!(GithubSourceDefinition::build(config()).is_ok());

        let mut missing_query = config();
        missing_query.query.clear();
        assert!(GithubSourceDefinition::build(missing_query).is_err());

        let mut missing_statuses = config();
        missing_statuses.status_map.clear();
        assert!(GithubSourceDefinition::build(missing_statuses).is_err());

        let mut empty_status = config();
        empty_status.status_map = BTreeMap::from([("ready".into(), " ".into())]);
        assert!(GithubSourceDefinition::build(empty_status).is_err());

        let mut zero_limit = config();
        zero_limit.limit = 0;
        assert!(GithubSourceDefinition::build(zero_limit).is_err());
    }

    #[test]
    fn reports_github_collection_metadata_and_host_bucket() {
        let source = GithubSourceDefinition::build(config()).unwrap();
        let collection = source
            .collection_from_items("github", Vec::new(), 123)
            .unwrap();

        assert_eq!(source.item_bucket_identity(), "github.com");
        assert_eq!(collection.available, Some(123));
        assert_eq!(collection.limit, 50);

        let issue = json!({
            "repository_url": "https://api.github.com/repos/zenobi-us/agentboard",
            "number": 42,
            "title": "Duplicate",
            "state": "open",
            "html_url": "https://github.com/zenobi-us/agentboard/issues/42",
            "labels": []
        });
        let item =
            normalize_issue("github", &issue, &Default::default(), &BTreeMap::new()).unwrap();
        assert!(source
            .collection_from_items("github", vec![item.clone(), item], 2)
            .is_err());
    }

    #[tokio::test]
    async fn cancellation_before_request_does_not_contact_github() {
        let (url, requests, _) = mock_server(Vec::new());
        let source = source_with_url(url);
        let cancellation = CancellationToken::new();
        cancellation.cancel();

        let error = source
            .health_check(&HealthCheckContext {
                source_id: "github",
                cancellation: cancellation.clone(),
            })
            .await
            .unwrap_err();

        assert!(error.to_string().contains("cancelled"));
        assert_eq!(requests.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn cancellation_during_response_drops_active_request() {
        let response = MockResponse {
            body: response_with_issues(vec![issue(1)]).body,
            delay_before_accept: Duration::ZERO,
            delay_before_response: Duration::from_secs(2),
        };
        let (url, requests, _) = mock_server(vec![response]);
        let source = source_with_url(url);
        let cancellation = CancellationToken::new();
        let task_cancellation = cancellation.clone();
        let task = tokio::spawn(async move {
            source
                .collect_github_issues("github", &task_cancellation)
                .await
        });

        wait_for_count(&requests, 1).await;
        let cancelled_at = Instant::now();
        cancellation.cancel();
        let deadline = cancelled_at + Duration::from_millis(200);
        let mut task = task;
        let result = loop {
            tokio::select! {
                result = &mut task => break result.unwrap(),
                _ = tokio::task::yield_now() => {}
            }
            assert!(
                Instant::now() < deadline,
                "github response cancellation was too slow"
            );
        };
        let error = result.unwrap_err();
        assert!(error.to_string().contains("cancelled"));
        assert!(cancelled_at.elapsed() < Duration::from_millis(200));
    }

    #[tokio::test]
    async fn cancellation_between_pages_does_not_request_later_page() {
        let first = response_with_issues(vec![issue(1)]);
        let second = MockResponse {
            delay_before_accept: Duration::from_millis(250),
            ..response_with_issues(vec![issue(2)])
        };
        let (url, requests, completed) = mock_server(vec![first, second]);
        let mut source = source_with_url(url);
        source.config.limit = 2;
        let cancellation = CancellationToken::new();
        let task_cancellation = cancellation.clone();
        let task = tokio::spawn(async move {
            source
                .collect_github_issues("github", &task_cancellation)
                .await
        });

        wait_for_count(&requests, 1).await;
        wait_for_count(&completed, 1).await;
        cancellation.cancel();

        let error = task.await.unwrap().unwrap_err();
        assert!(error.to_string().contains("cancelled"));
        assert_eq!(requests.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn completed_response_wins_before_cancellation() {
        const ITERATIONS: usize = 100;
        let responses = (0..ITERATIONS)
            .map(|_| response_with_issues(vec![issue(1)]))
            .collect();
        let (url, _, _) = mock_server(responses);
        let mut source = source_with_url(url);
        source.config.limit = 1;
        let client_completed = Arc::new(AtomicUsize::new(0));
        source.client_completed = Some(Arc::clone(&client_completed));
        let source = Arc::new(source);

        for expected in 1..=ITERATIONS {
            let cancellation = CancellationToken::new();
            let task_cancellation = cancellation.clone();
            let task_source = Arc::clone(&source);
            let task = tokio::spawn(async move {
                task_source
                    .collect_github_issues("github", &task_cancellation)
                    .await
            });

            wait_for_count(&client_completed, expected).await;
            cancellation.cancel();
            let collection = task.await.unwrap().unwrap();

            assert_eq!(collection.items.len(), 1);
            assert_eq!(collection.items[0].id, "zenobi-us/agentboard#1");
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cancellation_while_github_credential_helper_runs_kills_process_group() {
        let marker = std::env::temp_dir().join(format!(
            "agentboard-github-helper-{}.pid",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&marker);
        let marker_literal = marker.to_string_lossy().replace('\'', "'\\\"'\\\"'");
        let credentials = GithubCredentialConfig {
            helper: format!(
                "shell=$$; sleep 5 & child=$!; printf '%s %s' \"$shell\" \"$child\" > '{marker_literal}'; wait \"$child\""
            ),
        };
        let marker_path = marker;
        let cancellation = CancellationToken::new();
        let task_cancellation = cancellation.clone();
        let task =
            tokio::spawn(async move { github_token(&credentials, &task_cancellation).await });

        let started = Instant::now();
        let deadline = started + Duration::from_secs(1);
        while !marker_path.exists() {
            assert!(Instant::now() < deadline, "credential helper did not start");
            tokio::task::yield_now().await;
        }
        let pids = std::fs::read_to_string(&marker_path).unwrap();
        cancellation.cancel();

        let mut task = task;
        let error = loop {
            tokio::select! {
                result = &mut task => break result.unwrap().unwrap_err(),
                _ = tokio::task::yield_now() => {}
            }
            assert!(
                started.elapsed() < Duration::from_secs(1),
                "credential helper cancellation was too slow"
            );
        };
        assert!(error.to_string().contains("cancelled"));
        assert!(started.elapsed() < Duration::from_secs(1));

        for pid in pids.split_whitespace() {
            let deadline = Instant::now() + Duration::from_secs(1);
            while std::process::Command::new("kill")
                .args(["-0", pid])
                .stderr(std::process::Stdio::null())
                .status()
                .map(|status| status.success())
                .unwrap_or(false)
            {
                assert!(
                    Instant::now() < deadline,
                    "credential helper process remained alive"
                );
                tokio::task::yield_now().await;
            }
        }
        std::fs::remove_file(marker_path).unwrap();
    }

    #[tokio::test]
    async fn github_health_check_defers_credential_helper_until_runtime() {
        let source = GithubSourceDefinition::build(config()).unwrap();
        let context = HealthCheckContext {
            source_id: "github",
            cancellation: CancellationToken::new(),
        };

        assert!(source.health_check(&context).await.is_err());
    }

    #[test]
    fn injects_issue_search_guard() {
        assert_eq!(
            issue_only_query("repo:zenobi-us/agentboard is:open"),
            "is:issue repo:zenobi-us/agentboard is:open"
        );
        assert_eq!(
            issue_only_query("is:issue repo:zenobi-us/agentboard"),
            "is:issue repo:zenobi-us/agentboard"
        );
    }

    #[test]
    fn normalizes_issue_identity_and_status_label() {
        let mut statuses = BTreeMap::new();
        statuses.insert("ready".into(), "ready-for-agent".into());
        let issue = json!({
            "repository_url": "https://api.github.com/repos/zenobi-us/agentboard",
            "number": 42,
            "title": "Build github source",
            "state": "open",
            "html_url": "https://github.com/zenobi-us/agentboard/issues/42",
            "labels": [{"name": "ready"}]
        });

        let item = normalize_issue("gh", &issue, &Default::default(), &statuses).unwrap();
        assert_eq!(item.id, "zenobi-us/agentboard#42");
        assert_eq!(item.reference_id, "42");
        assert_eq!(item.status, "ready-for-agent");
        assert_eq!(item.source_kind, "github");
        assert_eq!(item.raw["github"]["issue"]["number"], 42);
    }

    #[test]
    fn supports_github_field_mapping() {
        let issue = json!({
            "repository_url": "https://api.github.com/repos/zenobi-us/agentboard",
            "number": 8,
            "title": "Original",
            "state": "open",
            "html_url": "https://github.com/zenobi-us/agentboard/issues/8",
            "labels": [],
            "custom": {"reference": "GH-8", "title": "Mapped"}
        });
        let field_map = agentboard_core::model::FieldMap {
            id: Some("custom.reference".into()),
            title: Some("custom.title".into()),
            ..Default::default()
        };

        let item = normalize_issue("gh", &issue, &field_map, &BTreeMap::new()).unwrap();
        assert_eq!(item.id, "zenobi-us/agentboard#8");
        assert_eq!(item.reference_id, "GH-8");
        assert_eq!(item.title, "Mapped");
    }

    #[test]
    fn same_issue_number_in_different_repositories_has_distinct_identity() {
        let issue = |repo: &str| {
            json!({
                "repository_url": format!("https://api.github.com/repos/{repo}"),
                "number": 8,
                "title": "Same number",
                "state": "open",
                "html_url": format!("https://github.com/{repo}/issues/8"),
                "labels": []
            })
        };

        let first = normalize_issue(
            "gh",
            &issue("zenobi-us/agentboard"),
            &Default::default(),
            &BTreeMap::new(),
        )
        .unwrap();
        let second = normalize_issue(
            "gh",
            &issue("zenobi-us/other"),
            &Default::default(),
            &BTreeMap::new(),
        )
        .unwrap();

        assert_ne!(first.id, second.id);
        assert_eq!(first.reference_id, second.reference_id);
    }

    #[test]
    fn falls_back_to_issue_state_without_mapped_label() {
        let issue = json!({
            "repository_url": "https://api.github.com/repos/zenobi-us/agentboard",
            "number": 7,
            "title": "No label",
            "state": "closed",
            "html_url": "https://github.com/zenobi-us/agentboard/issues/7",
            "labels": []
        });

        let item = normalize_issue("gh", &issue, &Default::default(), &BTreeMap::new()).unwrap();
        assert_eq!(item.status, "closed");
    }

    #[test]
    fn rejects_pull_requests_from_search() {
        let issue = json!({
            "repository_url": "https://api.github.com/repos/zenobi-us/agentboard",
            "number": 1,
            "title": "PR",
            "state": "open",
            "html_url": "https://github.com/zenobi-us/agentboard/pull/1",
            "pull_request": {}
        });

        assert!(normalize_issue("gh", &issue, &Default::default(), &BTreeMap::new()).is_err());
    }
}

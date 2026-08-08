use std::{
    collections::HashSet,
    env,
    process::{Command, Stdio},
};

#[cfg(test)]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(test)]
static CANCEL_BEFORE_COLLECTION_RETURN: AtomicBool = AtomicBool::new(false);

use anyhow::{anyhow, bail, Context, Result};
use command_group::AsyncCommandGroup;
use reqwest::Client;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use agentboard_core::{
    model::{FieldMap, Item},
    registry::{
        RuntimeResult, Source, SourceCollection, SourceContext, SourceDefinition, SourceFuture,
    },
};

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct JiraSourceConfig {
    pub site: String,
    #[serde(default = "default_jira_email_env")]
    pub email_env: String,
    #[serde(default = "default_jira_token_env")]
    pub token_env: String,
    #[serde(default)]
    pub credentials: Option<JiraCredentialConfig>,
    pub jql: String,
    #[serde(default = "default_source_limit")]
    pub limit: usize,
    #[serde(default)]
    pub fields: Vec<String>,
    #[serde(default)]
    pub field_map: FieldMap,
    #[serde(default)]
    pub status_map: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct JiraCredentialConfig {
    pub helper: String,
}

pub struct JiraSourceDefinition;

pub struct JiraSource {
    config: JiraSourceConfig,
    request_site: String,
}

impl SourceDefinition for JiraSourceDefinition {
    const ID: &'static str = "jira";
    type Config = JiraSourceConfig;
    type Runtime = JiraSource;

    fn build(mut config: Self::Config) -> RuntimeResult<Self::Runtime> {
        if config.site.trim().is_empty() {
            bail!("requires site");
        }
        let site = reqwest::Url::parse(config.site.trim()).context("site must be a valid URL")?;
        if !matches!(site.scheme(), "http" | "https") {
            bail!("site URL scheme must be http or https");
        }
        if let Some(credentials) = &config.credentials {
            if credentials.helper.trim().is_empty() {
                bail!("credential helper cannot be empty");
            }
        } else {
            if config.email_env.trim().is_empty() {
                bail!("requires email_env");
            }
            if config.token_env.trim().is_empty() {
                bail!("requires token_env");
            }
        }
        if config.jql.trim().is_empty() {
            bail!("requires jql");
        }
        if config.limit == 0 {
            bail!("limit must be greater than zero");
        }
        config.site = config.site.trim_end_matches('/').to_string();
        Ok(JiraSource {
            config,
            request_site: site.as_str().trim_end_matches('/').to_string(),
        })
    }
}

impl JiraSource {
    async fn collect_jira(
        &self,
        source_id: &str,
        cancellation: &agentboard_core::CancellationToken,
    ) -> Result<SourceCollection> {
        let site = self.request_site.as_str();
        let query = JiraQuery {
            email_env: &self.config.email_env,
            token_env: &self.config.token_env,
            credentials: self.config.credentials.as_ref(),
            jql: &self.config.jql,
            limit: self.config.limit,
            fields: &self.config.fields,
            field_map: &self.config.field_map,
        };
        check_jira_cancellation(cancellation, "jira collection")?;
        let credential = jira_credential(&query, site, cancellation).await?;
        let client = Client::new();
        let mut next_page_token = None;
        let mut available = None;
        let mut ids = HashSet::new();
        let mut items = Vec::new();

        while items.len() < query.limit {
            check_jira_cancellation(cancellation, "jira collection")?;
            let page_size = (query.limit - items.len()).min(100);
            let search = jira_search(
                &client,
                &format!("{site}/rest/api/3/search/jql"),
                &credential.username,
                &credential.password,
                query.jql,
                page_size,
                query.fields,
                query.field_map,
                next_page_token.as_deref(),
                cancellation,
            )
            .await?;
            available = available.or_else(|| {
                search
                    .get("total")
                    .and_then(Value::as_u64)
                    .map(|value| value as usize)
            });
            let issues = search
                .get("issues")
                .and_then(Value::as_array)
                .ok_or_else(|| anyhow!("jira search response missing issues array"))?;
            if issues.is_empty() {
                break;
            }

            for issue in issues {
                check_jira_cancellation(cancellation, "jira normalization")?;
                let item = normalize_issue(
                    self.config.site.as_str(),
                    source_id,
                    issue,
                    &self.config.field_map,
                    &self.config.status_map,
                )?;
                if !ids.insert(item.id.clone()) {
                    bail!("duplicate item id {} in source {source_id}", item.id);
                }
                items.push(item);
                if items.len() >= query.limit {
                    break;
                }
            }

            if items.len() >= query.limit {
                break;
            }
            let Some(token) = search
                .get("nextPageToken")
                .and_then(Value::as_str)
                .filter(|token| !token.is_empty())
            else {
                break;
            };
            if next_page_token.as_deref() == Some(token) {
                break;
            }
            next_page_token = Some(token.to_string());
        }

        #[cfg(test)]
        if CANCEL_BEFORE_COLLECTION_RETURN.swap(false, Ordering::AcqRel) {
            cancellation.cancel();
        }
        Ok(SourceCollection {
            items,
            available,
            limit: self.config.limit,
        })
    }

    #[cfg(test)]
    fn collection_from_search(&self, source_id: &str, search: Value) -> Result<SourceCollection> {
        self.collection_from_search_with_cancellation(
            source_id,
            search,
            &agentboard_core::CancellationToken::new(),
        )
    }

    #[cfg(test)]
    fn collection_from_search_with_cancellation(
        &self,
        source_id: &str,
        search: Value,
        cancellation: &agentboard_core::CancellationToken,
    ) -> Result<SourceCollection> {
        let issues = search
            .get("issues")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("jira search response missing issues array"))?;
        let site = self.config.site.as_str();
        let mut ids = HashSet::new();
        let mut items = Vec::new();
        for issue in issues {
            check_jira_cancellation(cancellation, "jira normalization")?;
            let item = normalize_issue(
                site,
                source_id,
                issue,
                &self.config.field_map,
                &self.config.status_map,
            )?;
            if !ids.insert(item.id.clone()) {
                bail!("duplicate item id {} in source {source_id}", item.id);
            }
            items.push(item);
        }
        check_jira_cancellation(cancellation, "jira normalization")?;
        Ok(SourceCollection {
            items,
            available: search
                .get("total")
                .and_then(Value::as_u64)
                .map(|value| value as usize),
            limit: self.config.limit,
        })
    }
}

impl Source for JiraSource {
    fn collect<'a>(&'a self, context: &'a SourceContext<'a>) -> SourceFuture<'a> {
        Box::pin(async move {
            self.collect_jira(context.source_id, &context.cancellation)
                .await
        })
    }

    fn item_bucket_identity(&self) -> String {
        normalize_site(&self.request_site)
    }
}

fn default_source_limit() -> usize {
    50
}

fn default_jira_email_env() -> String {
    "JIRA_EMAIL".into()
}

fn default_jira_token_env() -> String {
    "JIRA_API_TOKEN".into()
}

fn normalize_site(site: &str) -> String {
    site.trim()
        .trim_end_matches('/')
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .to_ascii_lowercase()
}

struct JiraQuery<'a> {
    email_env: &'a str,
    token_env: &'a str,
    credentials: Option<&'a JiraCredentialConfig>,
    jql: &'a str,
    limit: usize,
    fields: &'a [String],
    field_map: &'a FieldMap,
}

fn normalize_issue(
    site: &str,
    source_id: &str,
    issue: &Value,
    field_map: &FieldMap,
    status_map: &std::collections::BTreeMap<String, String>,
) -> Result<Item> {
    let id = mapped_field(issue, "id", "identity")?;
    let key = mapped_field(issue, "key", "reference id")?;
    let reference_id = match field_map.id.as_deref() {
        Some(path) => mapped_field(issue, path, "id")?,
        None => key.clone(),
    };
    let title = mapped_field(
        issue,
        field_map.title.as_deref().unwrap_or("fields.summary"),
        "title",
    )?;
    let status = mapped_field(
        issue,
        field_map.status.as_deref().unwrap_or("fields.status.name"),
        "status",
    )?;
    let status = mapped_status(&status, status_map);
    let url = match field_map.url.as_deref() {
        Some(path) => mapped_field(issue, path, "url")?,
        None => format!("{site}/browse/{key}"),
    };

    Ok(Item {
        id,
        reference_id,
        title,
        status,
        url,
        source_id: source_id.to_string(),
        source_kind: "jira".to_string(),
        raw: json!({ "jira": issue }),
    })
}

#[allow(clippy::too_many_arguments)]
async fn jira_search(
    client: &Client,
    url: &str,
    email: &str,
    token: &str,
    jql: &str,
    limit: usize,
    fields: &[String],
    map: &FieldMap,
    next_page_token: Option<&str>,
    cancellation: &agentboard_core::CancellationToken,
) -> Result<Value> {
    let requested_fields = jira_fetch_fields(fields, map);
    let mut body = json!({
        "jql": jql,
        "maxResults": limit,
        "fields": requested_fields,
    });
    if let Some(next_page_token) = next_page_token {
        body["nextPageToken"] = json!(next_page_token);
    }

    let request = client
        .post(url)
        .basic_auth(email, Some(token))
        .json(&body)
        .send();
    let response = tokio::select! {
        biased;
        response = request => response.context("send jira search request")?,
        _ = cancellation.cancelled() => bail!("jira search cancelled"),
    };
    let status = response.status();
    let text = tokio::select! {
        biased;
        text = response.text() => text.context("read jira search response")?,
        _ = cancellation.cancelled() => bail!("jira search cancelled"),
    };
    if !status.is_success() {
        bail!("jira search failed with {status}: {text}");
    }
    serde_json::from_str(&text).context("parse jira search JSON")
}

fn check_jira_cancellation(
    cancellation: &agentboard_core::CancellationToken,
    operation: &str,
) -> Result<()> {
    if cancellation.is_cancelled() {
        bail!("{operation} cancelled");
    }
    Ok(())
}

struct JiraCredential {
    username: String,
    password: String,
}

async fn jira_credential(
    query: &JiraQuery<'_>,
    site: &str,
    cancellation: &agentboard_core::CancellationToken,
) -> Result<JiraCredential> {
    if let Some(credentials) = query.credentials {
        let output = run_jira_credential_helper(
            &credentials.helper,
            &format!("protocol=https\nhost={}\n\n", site_host(site)),
            cancellation,
        )
        .await?;
        return parse_jira_credential(&output);
    }

    Ok(JiraCredential {
        username: env::var(query.email_env)
            .with_context(|| format!("read env {}", query.email_env))?,
        password: env::var(query.token_env)
            .with_context(|| format!("read env {}", query.token_env))?,
    })
}

async fn run_jira_credential_helper(
    helper: &str,
    stdin: &str,
    cancellation: &agentboard_core::CancellationToken,
) -> Result<String> {
    if helper.trim().is_empty() {
        bail!("jira credential helper cannot be empty");
    }

    let mut child = tokio::process::Command::from(shell_command(helper))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .group_spawn()
        .with_context(|| format!("run jira credential helper {helper}"))?;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut child_stdin = child
        .inner()
        .stdin
        .take()
        .context("open jira credential helper stdin")?;
    tokio::select! {
        biased;
        result = child_stdin.write_all(stdin.as_bytes()) => {
            result.context("write jira credential helper request")?;
        }
        _ = cancellation.cancelled() => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            bail!("jira credential helper cancelled");
        }
    }
    drop(child_stdin);
    let stdout = child
        .inner()
        .stdout
        .take()
        .ok_or_else(|| anyhow!("jira helper stdout was not captured"))?;
    let stderr = child
        .inner()
        .stderr
        .take()
        .ok_or_else(|| anyhow!("jira helper stderr was not captured"))?;
    let output = tokio::select! {
        biased;
        result = async {
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
            let _ = child.kill().await;
            let _ = child.wait().await;
            bail!("jira credential helper cancelled");
        }
    };
    if !output.status.success() {
        bail!(
            "jira credential helper failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
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

fn parse_jira_credential(output: &str) -> Result<JiraCredential> {
    let mut username = None;
    let mut password = None;
    for line in output.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key {
            "username" | "email" => username = Some(value.to_string()),
            "password" | "token" => password = Some(value.to_string()),
            _ => {}
        }
    }

    Ok(JiraCredential {
        username: username.ok_or_else(|| anyhow!("jira credential helper missing username"))?,
        password: password.ok_or_else(|| anyhow!("jira credential helper missing password"))?,
    })
}

fn site_host(site: &str) -> String {
    site.trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or(site)
        .to_string()
}

fn jira_fetch_fields(extra_fields: &[String], map: &FieldMap) -> Vec<String> {
    let mut fields = vec!["summary".to_string(), "status".to_string()];
    for path in [
        map.id.as_deref(),
        map.title.as_deref(),
        map.status.as_deref(),
        map.url.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        add_mapped_fetch_field(&mut fields, path);
    }
    for field in extra_fields {
        add_fetch_field(&mut fields, field);
    }
    fields
}

fn add_mapped_fetch_field(fields: &mut Vec<String>, path: &str) {
    let Some(field) = path
        .strip_prefix("fields.")
        .and_then(|rest| rest.split('.').next())
    else {
        return;
    };
    if !field.is_empty() {
        add_fetch_field(fields, field);
    }
}

fn add_fetch_field(fields: &mut Vec<String>, field: &str) {
    if !fields.iter().any(|existing| existing == field) {
        fields.push(field.to_string());
    }
}

fn mapped_field(issue: &Value, path: &str, name: &str) -> Result<String> {
    optional_mapped_field(issue, path)
        .ok_or_else(|| anyhow!("jira mapping {name}={path} must resolve to a string"))
}

fn mapped_status(status: &str, status_map: &std::collections::BTreeMap<String, String>) -> String {
    status_map
        .get(status)
        .cloned()
        .unwrap_or_else(|| status.to_string())
}

fn optional_mapped_field(issue: &Value, path: &str) -> Option<String> {
    let mut value = issue;
    for part in path.split('.') {
        value = value.get(part)?;
    }
    value.as_str().map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentboard_core::{
        registry::{HealthCheckContext, RawConfig, Registry, Source, SourceDefinition},
        CancellationToken,
    };
    use std::{
        io::{Read, Write},
        net::{SocketAddr, TcpListener, TcpStream},
        sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            Arc,
        },
        thread,
        time::Duration,
    };

    fn config(site: &str) -> JiraSourceConfig {
        serde_json::from_value(json!({
            "site": site,
            "jql": "project = AB"
        }))
        .unwrap()
    }

    fn mock_config(site: &str) -> JiraSourceConfig {
        serde_json::from_value(json!({
            "site": site,
            "jql": "project = AB",
            "credentials": {
                "helper": "printf 'username=user@example.com\\npassword=secret\\n'"
            }
        }))
        .unwrap()
    }

    fn jira_page(issues: Value) -> Value {
        json!({"issues": issues})
    }

    struct MockJiraServer {
        url: String,
        address: SocketAddr,
        connections: Arc<AtomicUsize>,
        body_started: Arc<AtomicBool>,
        stop: Arc<AtomicBool>,
        release: Arc<AtomicBool>,
        thread: Option<thread::JoinHandle<()>>,
    }

    impl MockJiraServer {
        fn start(responses: Vec<Value>, delayed_response: Option<usize>) -> Self {
            Self::start_with_body_delay(responses, delayed_response, None)
        }

        fn start_with_body_delay(
            responses: Vec<Value>,
            delayed_response: Option<usize>,
            delayed_body_response: Option<usize>,
        ) -> Self {
            let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
            let address = listener.local_addr().unwrap();
            let url = format!("http://{address}");
            let connections = Arc::new(AtomicUsize::new(0));
            let body_started = Arc::new(AtomicBool::new(false));
            let stop = Arc::new(AtomicBool::new(false));
            let release = Arc::new(AtomicBool::new(false));
            let thread_stop = Arc::clone(&stop);
            let thread_release = Arc::clone(&release);
            let thread_connections = Arc::clone(&connections);
            let thread_body_started = Arc::clone(&body_started);
            let thread = thread::spawn(move || {
                for (index, response) in responses.into_iter().enumerate() {
                    let Ok((mut stream, _)) = listener.accept() else {
                        return;
                    };
                    thread_connections.fetch_add(1, Ordering::Relaxed);
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    read_http_request(&mut stream);
                    if delayed_response == Some(index) {
                        while !thread_stop.load(Ordering::Relaxed)
                            && !thread_release.load(Ordering::Relaxed)
                        {
                            thread::sleep(Duration::from_millis(2));
                        }
                    }
                    let body = serde_json::to_vec(&response).unwrap();
                    let header = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(header.as_bytes());
                    if delayed_body_response == Some(index) {
                        let split = body.len().min(1);
                        let _ = stream.write_all(&body[..split]);
                        thread_body_started.store(true, Ordering::Release);
                        while !thread_stop.load(Ordering::Relaxed)
                            && !thread_release.load(Ordering::Relaxed)
                        {
                            thread::sleep(Duration::from_millis(2));
                        }
                        let _ = stream.write_all(&body[split..]);
                    } else {
                        let _ = stream.write_all(&body);
                    }
                }
            });
            Self {
                url,
                address,
                connections,
                body_started,
                stop,
                release,
                thread: Some(thread),
            }
        }

        async fn wait_for_connection(&self, expected: usize) {
            tokio::time::timeout(Duration::from_secs(2), async {
                while self.connections.load(Ordering::Relaxed) < expected {
                    tokio::time::sleep(Duration::from_millis(2)).await;
                }
            })
            .await
            .unwrap();
        }

        async fn wait_for_body(&self) {
            tokio::time::timeout(Duration::from_secs(2), async {
                while !self.body_started.load(Ordering::Acquire) {
                    tokio::time::sleep(Duration::from_millis(2)).await;
                }
            })
            .await
            .unwrap();
        }
    }

    impl Drop for MockJiraServer {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            self.release.store(true, Ordering::Relaxed);
            let _ = TcpStream::connect(self.address);
            if let Some(thread) = self.thread.take() {
                thread.join().unwrap();
            }
        }
    }

    fn read_http_request(stream: &mut TcpStream) {
        let mut bytes = Vec::new();
        let mut chunk = [0; 1024];
        loop {
            let Ok(count) = stream.read(&mut chunk) else {
                return;
            };
            if count == 0 {
                return;
            }
            bytes.extend_from_slice(&chunk[..count]);
            if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                thread::sleep(Duration::from_millis(50));
                return;
            }
        }
    }

    #[test]
    fn registers_jira_config_schema() {
        let mut registry = Registry::new();
        registry.add_source::<JiraSourceDefinition>().unwrap();

        let registration = registry.sources().next().unwrap();
        let schema = serde_json::to_value(registration.schema()).unwrap();
        let config = config("https://example.atlassian.net");

        assert_eq!(registration.id(), "jira");
        assert!(schema["properties"]["site"].is_object());
        assert!(schema["properties"]["jql"].is_object());
        assert_eq!(schema["properties"]["email_env"]["default"], "JIRA_EMAIL");
        assert_eq!(
            schema["properties"]["token_env"]["default"],
            "JIRA_API_TOKEN"
        );
        assert_eq!(schema["properties"]["limit"]["default"], 50);
        assert_eq!(schema["additionalProperties"], false);
        let required = schema["required"].as_array().unwrap();
        for field in ["site", "jql"] {
            assert!(required.iter().any(|value| value == field));
        }
        assert_eq!(config.email_env, "JIRA_EMAIL");
        assert_eq!(config.token_env, "JIRA_API_TOKEN");
        assert_eq!(config.limit, 50);

        let source = registry
            .build_source(
                "jira",
                serde_json::from_value::<RawConfig>(json!({
                    "site": "https://example.atlassian.net",
                    "jql": "project = AB"
                }))
                .unwrap(),
            )
            .unwrap();
        assert_eq!(source.item_bucket_identity(), "example.atlassian.net");
        assert!(registry
            .build_source(
                "jira",
                serde_json::from_value::<RawConfig>(json!({
                    "site": "https://example.atlassian.net",
                    "jql": "project = AB",
                    "extra": true
                }))
                .unwrap(),
            )
            .is_err());
    }

    #[test]
    fn deserializes_existing_jira_toml_fields() {
        let config: JiraSourceConfig = toml::from_str(
            r#"
                site = "https://example.atlassian.net"
                jql = "project = AB ORDER BY updated DESC"
                credentials = { helper = "agentboard-jira-credentials" }
                fields = ["assignee"]
                field_map = { id = "key", status = "fields.status.name" }
                status_map = { "To Do" = "ready" }
            "#,
        )
        .unwrap();

        assert!(JiraSourceDefinition::build(config).is_ok());
    }

    #[test]
    fn validates_jira_config_without_credential_lookup() {
        assert!(JiraSourceDefinition::build(config("https://example.atlassian.net")).is_ok());
        assert!(JiraSourceDefinition::build(config(" ")).is_err());

        let mut missing_jql = config("https://example.atlassian.net");
        missing_jql.jql.clear();
        assert!(JiraSourceDefinition::build(missing_jql).is_err());

        let mut zero_limit = config("https://example.atlassian.net");
        zero_limit.limit = 0;
        assert!(JiraSourceDefinition::build(zero_limit).is_err());

        let mut empty_helper = config("https://example.atlassian.net");
        empty_helper.credentials = Some(JiraCredentialConfig { helper: " ".into() });
        assert!(JiraSourceDefinition::build(empty_helper).is_err());
    }

    #[test]
    fn reports_jira_metadata_with_normalized_bucket_and_original_url_spelling() {
        let source = JiraSourceDefinition::build(config("HTTPS://Example.Atlassian.NET/")).unwrap();
        let same_site =
            JiraSourceDefinition::build(config("https://example.atlassian.net")).unwrap();
        let collection = source
            .collection_from_search(
                "jira",
                json!({
                    "issues": [{
                        "id": "10001",
                        "key": "AB-1",
                        "fields": {"summary": "Do it", "status": {"name": "Ready"}}
                    }]
                }),
            )
            .unwrap();

        assert_eq!(source.item_bucket_identity(), "example.atlassian.net");
        assert_eq!(
            source.item_bucket_identity(),
            same_site.item_bucket_identity()
        );
        assert_eq!(collection.items.len(), 1);
        assert_eq!(collection.available, None);
        assert_eq!(collection.limit, 50);
        assert_eq!(
            collection.items[0].url,
            "HTTPS://Example.Atlassian.NET/browse/AB-1"
        );
        assert_eq!(collection.items[0].raw["jira"]["id"], "10001");

        assert!(source
            .collection_from_search(
                "jira",
                json!({
                    "issues": [
                        {
                            "id": "10001",
                            "key": "AB-1",
                            "fields": {"summary": "One", "status": {"name": "Ready"}}
                        },
                        {
                            "id": "10001",
                            "key": "AB-2",
                            "fields": {"summary": "Two", "status": {"name": "Ready"}}
                        }
                    ]
                }),
            )
            .is_err());
    }

    #[tokio::test]
    async fn jira_health_check_defers_credential_helper_until_runtime() {
        let mut config = config("https://example.atlassian.net");
        config.credentials = Some(JiraCredentialConfig {
            helper: "exit 7".into(),
        });
        let source = JiraSourceDefinition::build(config).unwrap();
        let context = HealthCheckContext {
            source_id: "jira",
            cancellation: CancellationToken::new(),
        };

        assert!(source.health_check(&context).await.is_err());
    }

    #[tokio::test]
    async fn jira_collection_honors_pre_request_cancellation() {
        let server = MockJiraServer::start(vec![jira_page(json!([]))], None);
        let source = JiraSourceDefinition::build(mock_config(&server.url)).unwrap();
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let context = HealthCheckContext {
            source_id: "jira",
            cancellation,
        };

        let error = source.health_check(&context).await.unwrap_err();
        assert!(error.to_string().contains("cancelled"));
        assert_eq!(server.connections.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn jira_collection_drops_active_http_response_body_on_cancellation() {
        let server = MockJiraServer::start_with_body_delay(
            vec![jira_page(json!([{
                "id": "10001",
                "key": "AB-1",
                "fields": {"summary": "One", "status": {"name": "Ready"}}
            }]))],
            None,
            Some(0),
        );
        let source = JiraSourceDefinition::build(mock_config(&server.url)).unwrap();
        let cancellation = CancellationToken::new();
        let task_cancellation = cancellation.clone();
        let task = tokio::spawn(async move {
            let context = SourceContext {
                source_id: "jira",
                cancellation: task_cancellation,
            };
            source.collect(&context).await
        });

        server.wait_for_body().await;
        tokio::task::yield_now().await;
        cancellation.cancel();
        let result = tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap();
        assert!(result.unwrap_err().to_string().contains("cancelled"));
    }

    #[tokio::test]
    async fn jira_collection_cancels_an_active_later_page() {
        let first = json!({
            "issues": [{
                "id": "10001",
                "key": "AB-1",
                "fields": {"summary": "One", "status": {"name": "Ready"}}
            }],
            "nextPageToken": "page-2"
        });
        let second = jira_page(json!([{
            "id": "10002",
            "key": "AB-2",
            "fields": {"summary": "Two", "status": {"name": "Ready"}}
        }]));
        let server = MockJiraServer::start(vec![first, second], Some(1));
        let mut source_config = mock_config(&server.url);
        source_config.limit = 2;
        let source = JiraSourceDefinition::build(source_config).unwrap();
        let cancellation = CancellationToken::new();
        let task_cancellation = cancellation.clone();
        let task = tokio::spawn(async move {
            let context = SourceContext {
                source_id: "jira",
                cancellation: task_cancellation,
            };
            source.collect(&context).await
        });

        server.wait_for_connection(1).await;
        server.wait_for_connection(2).await;
        cancellation.cancel();
        let result = tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap();
        assert!(result.unwrap_err().to_string().contains("cancelled"));
        assert_eq!(server.connections.load(Ordering::Relaxed), 2);
    }

    #[tokio::test]
    async fn jira_collection_wins_completion_race() {
        let first = json!({
            "issues": [{
                "id": "10001",
                "key": "AB-1",
                "fields": {"summary": "One", "status": {"name": "Ready"}}
            }],
            "nextPageToken": "page-2"
        });
        let second = jira_page(json!([{
            "id": "10002",
            "key": "AB-2",
            "fields": {"summary": "Two", "status": {"name": "Ready"}}
        }]));
        let server = MockJiraServer::start(vec![first, second], None);
        let mut source_config = mock_config(&server.url);
        source_config.limit = 2;
        let source = JiraSourceDefinition::build(source_config).unwrap();
        let cancellation = CancellationToken::new();
        let context = SourceContext {
            source_id: "jira",
            cancellation: cancellation.clone(),
        };

        CANCEL_BEFORE_COLLECTION_RETURN.store(true, Ordering::Release);
        let collection = source.collect(&context).await.unwrap();
        assert_eq!(collection.items.len(), 2);
        assert_eq!(collection.items[1].reference_id, "AB-2");
    }

    #[test]
    fn parses_jira_credential_helper_output() {
        let credential = parse_jira_credential("email=user@example.com\ntoken=secret\n").unwrap();
        assert_eq!(credential.username, "user@example.com");
        assert_eq!(credential.password, "secret");
    }

    #[test]
    fn extracts_jira_site_host() {
        assert_eq!(
            site_host("https://example.atlassian.net/foo"),
            "example.atlassian.net"
        );
    }

    #[test]
    fn supports_nested_jira_field_mapping() {
        let issue = json!({"key":"AB-1","fields":{"summary":"Do it","status":{"name":"Ready"}}});
        assert_eq!(mapped_field(&issue, "key", "id").unwrap(), "AB-1");
        assert_eq!(
            mapped_field(&issue, "fields.status.name", "status").unwrap(),
            "Ready"
        );
    }

    #[test]
    fn normalizes_jira_identity_and_reference() {
        let issue = json!({
            "id": "10001",
            "key": "AB-1",
            "fields": {"summary": "Do it", "status": {"name": "Ready"}}
        });

        let item = normalize_issue(
            "https://example.atlassian.net",
            "jira",
            &issue,
            &Default::default(),
            &Default::default(),
        )
        .unwrap();

        assert_eq!(item.id, "10001");
        assert_eq!(item.reference_id, "AB-1");
        assert_eq!(item.url, "https://example.atlassian.net/browse/AB-1");
    }

    #[test]
    fn jira_field_map_id_changes_reference_not_identity() {
        let issue = json!({
            "id": "10001",
            "key": "AB-1",
            "fields": {
                "summary": "Do it",
                "status": {"name": "Ready"},
                "customfield_10010": "customer-42"
            }
        });
        let field_map = FieldMap {
            id: Some("fields.customfield_10010".into()),
            ..Default::default()
        };

        let item = normalize_issue(
            "https://example.atlassian.net",
            "jira",
            &issue,
            &field_map,
            &Default::default(),
        )
        .unwrap();

        assert_eq!(item.id, "10001");
        assert_eq!(item.reference_id, "customer-42");
    }

    #[test]
    fn maps_jira_status_values() {
        let status_map = std::collections::BTreeMap::from([("To Do".into(), "ready".into())]);
        assert_eq!(mapped_status("To Do", &status_map), "ready");
        assert_eq!(mapped_status("Done", &status_map), "Done");
    }

    #[test]
    fn infers_jira_fetch_fields_from_mapping_paths() {
        let map = FieldMap {
            id: Some("key".into()),
            title: Some("fields.customfield_10010".into()),
            status: Some("fields.parent.fields.status".into()),
            url: Some("fields.customfield_10020".into()),
        };

        assert_eq!(
            jira_fetch_fields(&["assignee".into(), "summary".into()], &map),
            vec![
                "summary",
                "status",
                "customfield_10010",
                "parent",
                "customfield_10020",
                "assignee"
            ]
        );
    }
}

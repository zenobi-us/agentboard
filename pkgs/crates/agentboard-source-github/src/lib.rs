use std::{collections::HashSet, process::Command};

use agentboard_core::{
    model::{GithubSourceMode as LegacyGithubSourceMode, Item, SourceConfig, SourceKind},
    registry::{
        RuntimeResult, Source, SourceCollection, SourceContext, SourceDefinition, SourceFuture,
    },
};
use anyhow::{anyhow, bail, Context, Result};
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
        Ok(GithubSource { config })
    }
}

impl GithubSource {
    async fn collect_github_issues(&self, source_id: &str) -> Result<SourceCollection> {
        let token = github_token(&self.config.credentials)?;
        let client = Client::new();
        let search_query = issue_only_query(&self.config.query);
        eprintln!("github source {source_id} query: {search_query}");
        let mut page = 1usize;
        let mut items = Vec::new();
        let mut available = None;

        while items.len() < self.config.limit {
            let page_size = (self.config.limit - items.len()).min(100);
            let response =
                github_issue_search(&client, &token, &search_query, page_size, page).await?;
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
                let item = normalize_issue(
                    source_id,
                    issue,
                    &self.config.field_map,
                    &self.config.status_map,
                )?;
                items.push(item);
                if items.len() >= self.config.limit {
                    break;
                }
            }
            page += 1;
        }

        self.collection_from_items(source_id, items, available.unwrap_or(0))
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
                GithubSourceMode::Issue => self.collect_github_issues(context.source_id).await,
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

/// Temporary legacy bridge for the CLI cutover in issue #24.
pub async fn collect_items(source: &SourceConfig) -> Result<Vec<Item>> {
    Ok(inspect_items(source).await?.0)
}

/// Collect configured Items and return GitHub's total matching issue count.
pub async fn inspect_items(source: &SourceConfig) -> Result<(Vec<Item>, usize)> {
    let config = match &source.source {
        SourceKind::Github {
            mode: LegacyGithubSourceMode::Issue,
            query,
            credentials,
            limit,
            field_map,
            status_map,
        } => GithubSourceConfig {
            mode: GithubSourceMode::Issue,
            query: query.clone(),
            credentials: GithubCredentialConfig {
                helper: credentials.helper.clone(),
            },
            limit: *limit,
            field_map: field_map.clone(),
            status_map: status_map.clone(),
        },
        _ => bail!("source {} is not github", source.id),
    };
    let collection = GithubSourceDefinition::build(config)?
        .collect_github_issues(&source.id)
        .await?;
    Ok((collection.items, collection.available.unwrap_or(0)))
}

async fn github_issue_search(
    client: &Client,
    token: &str,
    query: &str,
    per_page: usize,
    page: usize,
) -> Result<Value> {
    let response = client
        .get(GITHUB_SEARCH_URL)
        .bearer_auth(token)
        .header(header::ACCEPT, "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header(header::USER_AGENT, "agentboard")
        .query(&[
            ("q", query),
            ("per_page", &per_page.to_string()),
            ("page", &page.to_string()),
        ])
        .send()
        .await
        .context("send github issue search request")?;
    let status = response.status();
    let text = response
        .text()
        .await
        .context("read github issue search response")?;
    if !status.is_success() {
        bail!("github issue search failed with {status}: {text}");
    }
    serde_json::from_str(&text).context("parse github issue search JSON")
}

fn normalize_issue(
    source_id: &str,
    issue: &Value,
    field_map: &agentboard_core::model::FieldMap,
    status_map: &std::collections::BTreeMap<String, String>,
) -> Result<Item> {
    if issue.get("pull_request").is_some() {
        bail!("github issue search returned pull request; query must exclude pull requests");
    }

    let repo_url = string_field(
        issue.pointer("/repository_url"),
        "github issue repository_url",
    )?;
    let repo = repo_url
        .strip_prefix("https://api.github.com/repos/")
        .ok_or_else(|| anyhow!("github issue repository_url has unexpected format"))?;
    let number = issue
        .get("number")
        .and_then(Value::as_i64)
        .ok_or_else(|| anyhow!("github issue number must be an integer"))?;
    let id = format!("{repo}#{number}");
    let reference_id = match field_map.id.as_deref() {
        Some(path) => mapped_field(issue, path, "id")?,
        None => number.to_string(),
    };
    let title = mapped_field(
        issue,
        field_map.title.as_deref().unwrap_or("title"),
        "title",
    )?;
    let state = mapped_field(
        issue,
        field_map.status.as_deref().unwrap_or("state"),
        "status",
    )?;
    let url = mapped_field(issue, field_map.url.as_deref().unwrap_or("html_url"), "url")?;
    let status = mapped_status(issue, status_map)
        .unwrap_or_else(|| status_map.get(&state).cloned().unwrap_or(state));

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

fn github_token(credentials: &GithubCredentialConfig) -> Result<String> {
    if credentials.helper.trim().is_empty() {
        bail!("github credential helper cannot be empty");
    }
    let output = shell_command(&credentials.helper)
        .output()
        .with_context(|| format!("run github credential helper {}", credentials.helper))?;
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
    use agentboard_core::registry::{RawConfig, Registry, Source, SourceContext, SourceDefinition};
    use std::collections::BTreeMap;
    use std::{
        future::Future,
        task::{Context as TaskContext, Poll, Waker},
    };

    fn poll_ready<T>(future: impl Future<Output = T>) -> T {
        let mut context = TaskContext::from_waker(Waker::noop());
        let mut future = Box::pin(future);
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("future unexpectedly pending"),
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

    #[test]
    fn github_health_check_defers_credential_helper_until_runtime() {
        let source = GithubSourceDefinition::build(config()).unwrap();
        let context = SourceContext {
            source_id: "github",
        };

        assert!(poll_ready(source.health_check(&context)).is_err());
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

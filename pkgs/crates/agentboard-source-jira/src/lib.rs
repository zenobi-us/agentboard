use std::{
    collections::HashSet,
    env,
    io::Write,
    process::{Command, Stdio},
};

use anyhow::{anyhow, bail, Context, Result};
use reqwest::Client;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use agentboard_core::{
    model::{FieldMap, Item, SourceConfig, SourceKind},
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
        config.site = site.as_str().trim_end_matches('/').to_string();
        Ok(JiraSource { config })
    }
}

impl JiraSource {
    async fn collect_jira(&self, source_id: &str) -> Result<SourceCollection> {
        let site = self.config.site.as_str();
        let query = JiraQuery {
            email_env: &self.config.email_env,
            token_env: &self.config.token_env,
            credentials: self.config.credentials.as_ref(),
            jql: &self.config.jql,
            limit: self.config.limit,
            fields: &self.config.fields,
            field_map: &self.config.field_map,
        };
        let credential = jira_credential(&query, site)?;
        let search = jira_search(
            &format!("{site}/rest/api/3/search/jql"),
            &credential.username,
            &credential.password,
            query.jql,
            query.limit,
            query.fields,
            query.field_map,
        )
        .await?;
        self.collection_from_search(source_id, search)
    }

    fn collection_from_search(&self, source_id: &str, search: Value) -> Result<SourceCollection> {
        let issues = search
            .get("issues")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("jira search response missing issues array"))?;
        let site = self.config.site.as_str();
        let mut ids = HashSet::new();
        let mut items = Vec::new();
        for issue in issues {
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
        Ok(SourceCollection {
            items,
            available: None,
            limit: self.config.limit,
        })
    }
}

impl Source for JiraSource {
    fn collect<'a>(&'a self, context: &'a SourceContext<'a>) -> SourceFuture<'a> {
        Box::pin(async move { self.collect_jira(context.source_id).await })
    }

    fn item_bucket_identity(&self) -> String {
        normalize_site(&self.config.site)
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

/// Temporary legacy bridge for the CLI cutover in issue #24.
pub async fn collect_items(source: &SourceConfig) -> Result<Vec<Item>> {
    let config = match &source.source {
        SourceKind::Jira {
            site,
            email_env,
            token_env,
            credentials,
            jql,
            limit,
            fields,
            field_map,
            status_map,
        } => JiraSourceConfig {
            site: site.clone(),
            email_env: email_env.clone(),
            token_env: token_env.clone(),
            credentials: credentials
                .as_ref()
                .map(|credentials| JiraCredentialConfig {
                    helper: credentials.helper.clone(),
                }),
            jql: jql.clone(),
            limit: *limit,
            fields: fields.clone(),
            field_map: field_map.clone(),
            status_map: status_map.clone(),
        },
        _ => bail!("source {} is not jira", source.id),
    };
    Ok(JiraSourceDefinition::build(config)?
        .collect_jira(&source.id)
        .await?
        .items)
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

async fn jira_search(
    url: &str,
    email: &str,
    token: &str,
    jql: &str,
    limit: usize,
    fields: &[String],
    map: &FieldMap,
) -> Result<Value> {
    let requested_fields = jira_fetch_fields(fields, map);

    let response = Client::new()
        .post(url)
        .basic_auth(email, Some(token))
        .json(&json!({
            "jql": jql,
            "maxResults": limit,
            "fields": requested_fields,
        }))
        .send()
        .await
        .context("send jira search request")?;
    let status = response.status();
    let text = response.text().await.context("read jira search response")?;
    if !status.is_success() {
        bail!("jira search failed with {status}: {text}");
    }
    serde_json::from_str(&text).context("parse jira search JSON")
}

struct JiraCredential {
    username: String,
    password: String,
}

fn jira_credential(query: &JiraQuery<'_>, site: &str) -> Result<JiraCredential> {
    if let Some(credentials) = query.credentials {
        let output = run_jira_credential_helper(
            &credentials.helper,
            &format!("protocol=https\nhost={}\n\n", site_host(site)),
        )?;
        return parse_jira_credential(&output);
    }

    Ok(JiraCredential {
        username: env::var(query.email_env)
            .with_context(|| format!("read env {}", query.email_env))?,
        password: env::var(query.token_env)
            .with_context(|| format!("read env {}", query.token_env))?,
    })
}

fn run_jira_credential_helper(helper: &str, stdin: &str) -> Result<String> {
    if helper.trim().is_empty() {
        bail!("jira credential helper cannot be empty");
    }

    let mut child = shell_command(helper)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .with_context(|| format!("run jira credential helper {helper}"))?;

    child
        .stdin
        .as_mut()
        .context("open jira credential helper stdin")?
        .write_all(stdin.as_bytes())
        .context("write jira credential helper request")?;

    let output = child
        .wait_with_output()
        .context("read jira credential helper output")?;
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
    use agentboard_core::registry::{RawConfig, Registry, Source, SourceContext, SourceDefinition};
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

    fn config(site: &str) -> JiraSourceConfig {
        serde_json::from_value(json!({
            "site": site,
            "jql": "project = AB"
        }))
        .unwrap()
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
    fn reports_jira_collection_metadata_and_normalized_site_bucket() {
        let source =
            JiraSourceDefinition::build(config(" HTTPS://Example.Atlassian.NET/ ")).unwrap();
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
            "https://example.atlassian.net/browse/AB-1"
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

    #[test]
    fn jira_health_check_defers_credential_helper_until_runtime() {
        let mut config = config("https://example.atlassian.net");
        config.credentials = Some(JiraCredentialConfig {
            helper: "exit 7".into(),
        });
        let source = JiraSourceDefinition::build(config).unwrap();
        let context = SourceContext { source_id: "jira" };

        assert!(poll_ready(source.health_check(&context)).is_err());
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

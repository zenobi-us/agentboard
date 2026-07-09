use std::{
    collections::HashSet,
    env,
    io::Write,
    process::{Command, Stdio},
};

use anyhow::{anyhow, bail, Context, Result};
use reqwest::Client;
use serde_json::{json, Value};

use agentboard_core::model::{FieldMap, Item, JiraCredentialConfig, SourceConfig, SourceKind};

pub async fn collect_items(source: &SourceConfig) -> Result<Vec<Item>> {
    match &source.source {
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
        } => {
            collect_jira(
                &source.id,
                JiraQuery {
                    site,
                    email_env,
                    token_env,
                    credentials: credentials.as_ref(),
                    jql,
                    limit: *limit,
                    fields,
                    field_map,
                    status_map,
                },
            )
            .await
        }
        _ => bail!("source {} is not jira", source.id),
    }
}

struct JiraQuery<'a> {
    site: &'a str,
    email_env: &'a str,
    token_env: &'a str,
    credentials: Option<&'a JiraCredentialConfig>,
    jql: &'a str,
    limit: usize,
    fields: &'a [String],
    field_map: &'a FieldMap,
    status_map: &'a std::collections::BTreeMap<String, String>,
}

async fn collect_jira(source_id: &str, query: JiraQuery<'_>) -> Result<Vec<Item>> {
    let site = query.site.trim_end_matches('/');
    let credential = jira_credential(&query, site)?;
    let url = format!("{site}/rest/api/3/search/jql");
    let search = jira_search(
        &url,
        &credential.username,
        &credential.password,
        query.jql,
        query.limit,
        query.fields,
        query.field_map,
    )
    .await?;
    let issues = search
        .get("issues")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("jira search response missing issues array"))?;

    let mut ids = HashSet::new();
    let mut out = Vec::new();
    for issue in issues {
        let id = mapped_field(issue, query.field_map.id.as_deref().unwrap_or("key"), "id")?;
        if !ids.insert(id.clone()) {
            bail!("duplicate item id {id} in source {source_id}");
        }
        let title = mapped_field(
            issue,
            query.field_map.title.as_deref().unwrap_or("fields.summary"),
            "title",
        )?;
        let status = mapped_field(
            issue,
            query
                .field_map
                .status
                .as_deref()
                .unwrap_or("fields.status.name"),
            "status",
        )?;
        let status = mapped_status(&status, query.status_map);
        let url = match query.field_map.url.as_deref() {
            Some(path) => mapped_field(issue, path, "url")?,
            None => format!("{site}/browse/{id}"),
        };

        out.push(Item {
            id,
            title,
            status,
            url,
            source_id: source_id.to_string(),
            source_kind: "jira".to_string(),
            raw: json!({ "jira": issue }),
        });
    }
    Ok(out)
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

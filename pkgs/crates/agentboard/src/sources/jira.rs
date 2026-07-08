use std::{collections::HashSet, env};

use anyhow::{anyhow, bail, Context, Result};
use reqwest::Client;
use serde_json::{json, Value};

use crate::{
    model::{FieldMap, Item, SourceConfig, SourceKind},
    sources::SourceAdapter,
};

pub struct JiraSource;

impl SourceAdapter for JiraSource {
    async fn collect(&self, source: &SourceConfig) -> Result<Vec<Item>> {
        match &source.source {
            SourceKind::Jira {
                site,
                email_env,
                token_env,
                jql,
                limit,
                fields,
                map,
            } => {
                collect_jira(
                    &source.id,
                    JiraQuery {
                        site,
                        email_env,
                        token_env,
                        jql,
                        limit: *limit,
                        fields,
                        map,
                    },
                )
                .await
            }
            _ => bail!("source {} is not jira", source.id),
        }
    }
}

struct JiraQuery<'a> {
    site: &'a str,
    email_env: &'a str,
    token_env: &'a str,
    jql: &'a str,
    limit: usize,
    fields: &'a [String],
    map: &'a FieldMap,
}

async fn collect_jira(source_id: &str, query: JiraQuery<'_>) -> Result<Vec<Item>> {
    let email =
        env::var(query.email_env).with_context(|| format!("read env {}", query.email_env))?;
    let token =
        env::var(query.token_env).with_context(|| format!("read env {}", query.token_env))?;
    let site = query.site.trim_end_matches('/');
    let url = format!("{site}/rest/api/3/search/jql");
    let search = jira_search(&url, &email, &token, query.jql, query.limit, query.fields).await?;
    let issues = search
        .get("issues")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("jira search response missing issues array"))?;

    let mut ids = HashSet::new();
    let mut out = Vec::new();
    for issue in issues {
        let id = mapped_field(issue, query.map.id.as_deref().unwrap_or("key"), "id")?;
        if !ids.insert(id.clone()) {
            bail!("duplicate item id {id} in source {source_id}");
        }
        let title = mapped_field(
            issue,
            query.map.title.as_deref().unwrap_or("fields.summary"),
            "title",
        )?;
        let status = mapped_field(
            issue,
            query.map.status.as_deref().unwrap_or("fields.status.name"),
            "status",
        )?;
        let url = match query.map.url.as_deref() {
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
) -> Result<Value> {
    let mut requested_fields = vec!["summary".to_string(), "status".to_string()];
    for field in fields {
        if !requested_fields.contains(field) {
            requested_fields.push(field.clone());
        }
    }

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

fn mapped_field(issue: &Value, path: &str, name: &str) -> Result<String> {
    optional_mapped_field(issue, path)
        .ok_or_else(|| anyhow!("jira mapping {name}={path} must resolve to a string"))
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
    fn supports_nested_jira_field_mapping() {
        let issue = json!({"key":"AB-1","fields":{"summary":"Do it","status":{"name":"Ready"}}});
        assert_eq!(mapped_field(&issue, "key", "id").unwrap(), "AB-1");
        assert_eq!(
            mapped_field(&issue, "fields.status.name", "status").unwrap(),
            "Ready"
        );
    }
}

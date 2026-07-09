use std::{collections::HashSet, process::Command};

use agentboard_core::model::{
    GithubCredentialConfig, GithubSourceMode, Item, SourceConfig, SourceKind,
};
use anyhow::{anyhow, bail, Context, Result};
use reqwest::{header, Client};
use serde_json::{json, Value};

const GITHUB_SEARCH_URL: &str = "https://api.github.com/search/issues";

pub async fn collect_items(source: &SourceConfig) -> Result<Vec<Item>> {
    match &source.source {
        SourceKind::Github { mode } => match mode {
            GithubSourceMode::Issue {
                query,
                credentials,
                limit,
                status_labels,
            } => {
                collect_github_issues(
                    &source.id,
                    IssueQuery {
                        query,
                        credentials,
                        limit: *limit,
                        status_labels,
                    },
                )
                .await
            }
        },
        _ => bail!("source {} is not github", source.id),
    }
}

struct IssueQuery<'a> {
    query: &'a str,
    credentials: &'a GithubCredentialConfig,
    limit: usize,
    status_labels: &'a std::collections::BTreeMap<String, String>,
}

async fn collect_github_issues(source_id: &str, query: IssueQuery<'_>) -> Result<Vec<Item>> {
    let token = github_token(query.credentials)?;
    let client = Client::new();
    let search_query = issue_only_query(query.query);
    let mut page = 1usize;
    let mut out = Vec::new();
    let mut ids = HashSet::new();

    while out.len() < query.limit {
        let page_size = (query.limit - out.len()).min(100);
        let response = github_issue_search(&client, &token, &search_query, page_size, page).await?;
        let issues = response
            .get("items")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("github issue search response missing items array"))?;
        if issues.is_empty() {
            break;
        }

        for issue in issues {
            let item = normalize_issue(source_id, issue, query.status_labels)?;
            if !ids.insert(item.id.clone()) {
                bail!("duplicate item id {} in source {source_id}", item.id);
            }
            out.push(item);
            if out.len() >= query.limit {
                break;
            }
        }
        page += 1;
    }

    Ok(out)
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
    status_labels: &std::collections::BTreeMap<String, String>,
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
    let title = string_field(issue.get("title"), "github issue title")?;
    let state = string_field(issue.get("state"), "github issue state")?;
    let url = string_field(issue.get("html_url"), "github issue html_url")?;
    let status = mapped_status(issue, status_labels).unwrap_or_else(|| state.clone());

    Ok(Item {
        id,
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
    status_labels: &std::collections::BTreeMap<String, String>,
) -> Option<String> {
    let labels = issue.get("labels")?.as_array()?;
    for label in labels {
        let Some(name) = label.get("name").and_then(Value::as_str) else {
            continue;
        };
        if let Some(status) = status_labels.get(name) {
            return Some(status.clone());
        }
    }
    None
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
    use std::collections::BTreeMap;

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

        let item = normalize_issue("gh", &issue, &statuses).unwrap();
        assert_eq!(item.id, "zenobi-us/agentboard#42");
        assert_eq!(item.status, "ready-for-agent");
        assert_eq!(item.source_kind, "github");
        assert_eq!(item.raw["github"]["issue"]["number"], 42);
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

        let item = normalize_issue("gh", &issue, &BTreeMap::new()).unwrap();
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

        assert!(normalize_issue("gh", &issue, &BTreeMap::new()).is_err());
    }
}

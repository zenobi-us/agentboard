use std::{collections::HashSet, env, fs, path::PathBuf};

use agentboard_core::model::{
    ActionConfig, GithubSourceMode, SourceConfig, SourceKind, Workspace, WorkspaceConfig,
};
use anyhow::{bail, Context, Result};
use directories::BaseDirs;
use serde_json::Value;
use sha2::{Digest, Sha256};

/// Load a workspace by name or explicit TOML path, then validate it.
///
/// Named workspaces resolve under the user config directory. Explicit paths get
/// a stable workspace id from the file stem plus canonical path hash.
pub fn load_workspace(input: &str) -> Result<Workspace> {
    let path = if input.ends_with(".toml") || input.contains('/') {
        expand_path(input)
    } else {
        config_home()
            .join("agentboard")
            .join(format!("{input}.toml"))
    };
    let text =
        fs::read_to_string(&path).with_context(|| format!("read workspace {}", path.display()))?;
    let config: WorkspaceConfig = toml::from_str(&text)?;
    validate_config(&config)?;
    let id = if input.ends_with(".toml") || input.contains('/') {
        let canon = fs::canonicalize(&path)?;
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("workspace");
        format!("{stem}-{}", short_hash(&canon.display().to_string()))
    } else {
        input.to_string()
    };
    Ok(Workspace { id, path, config })
}

/// Validate workspace config invariants before a run touches sources or actions.
pub fn validate_config(config: &WorkspaceConfig) -> Result<()> {
    let mut ids = HashSet::new();
    for src in &config.sources {
        if !ids.insert(&src.id) {
            bail!("duplicate source id {}", src.id);
        }
        if src.id.trim().is_empty() {
            bail!("source id cannot be empty");
        }
        match &src.source {
            SourceKind::Qmd {
                collections,
                query,
                limit,
                ..
            } => {
                if collections.is_empty() {
                    bail!("qmd source {} requires at least one collection", src.id);
                }
                if query.trim().is_empty() {
                    bail!("qmd source {} requires query", src.id);
                }
                if *limit == 0 {
                    bail!("qmd source {} limit must be greater than zero", src.id);
                }
            }
            SourceKind::Jira {
                site,
                email_env,
                token_env,
                credentials,
                jql,
                limit,
                ..
            } => {
                if site.trim().is_empty() {
                    bail!("jira source {} requires site", src.id);
                }
                if let Some(credentials) = credentials {
                    if credentials.helper.trim().is_empty() {
                        bail!("jira source {} credential helper cannot be empty", src.id);
                    }
                } else {
                    if email_env.trim().is_empty() {
                        bail!("jira source {} requires email_env", src.id);
                    }
                    if token_env.trim().is_empty() {
                        bail!("jira source {} requires token_env", src.id);
                    }
                }
                if jql.trim().is_empty() {
                    bail!("jira source {} requires jql", src.id);
                }
                if *limit == 0 {
                    bail!("jira source {} limit must be greater than zero", src.id);
                }
            }
            SourceKind::Github {
                mode: GithubSourceMode::Issue,
                query,
                credentials,
                limit,
                status_map,
                ..
            } => {
                if query.trim().is_empty() {
                    bail!("github source {} requires query", src.id);
                }
                if credentials.helper.trim().is_empty() {
                    bail!("github source {} credential helper cannot be empty", src.id);
                }
                if status_map.is_empty() {
                    bail!("github source {} requires status_map", src.id);
                }
                for (label, status) in status_map {
                    if label.trim().is_empty() || status.trim().is_empty() {
                        bail!(
                            "github source {} status_map cannot contain empty labels or statuses",
                            src.id
                        );
                    }
                }
                if *limit == 0 {
                    bail!("github source {} limit must be greater than zero", src.id);
                }
            }
        }
        for action in &src.actions {
            match action.uses.as_str() {
                "agentboard/create-worktree" => require_inputs(action, &["repo", "root", "branch"]),
                "agentboard/run-cmd" => require_inputs(action, &["cmd"]),
                other if other.starts_with("agentboard/") => {
                    bail!("unknown built-in action {other}")
                }
                other => bail!("unknown action {other}"),
            }?;
        }
    }
    Ok(())
}

fn require_inputs(action: &ActionConfig, keys: &[&str]) -> Result<()> {
    for key in keys {
        if !action.inputs.contains_key(*key) {
            bail!("{} requires input {key}", action.uses);
        }
    }
    Ok(())
}

/// Return the XDG config directory used for named workspace files.
pub fn config_home() -> PathBuf {
    BaseDirs::new()
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".config"))
}

/// Return the XDG data directory used for append-only store records.
pub fn data_home() -> PathBuf {
    BaseDirs::new()
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".local/share"))
}

/// Return the user's home directory for `~` expansion.
pub fn home_dir() -> PathBuf {
    BaseDirs::new()
        .map(|d| d.home_dir().to_path_buf())
        .or_else(|| env::var_os("HOME").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Return the root store directory for one workspace.
pub fn store_root(ws: &Workspace) -> PathBuf {
    data_home().join("agentboard").join(&ws.id)
}

/// Return the per-source store directory for one workspace source.
pub fn source_dir(ws: &Workspace, source_id: &str) -> PathBuf {
    store_root(ws).join("sources").join(source_id)
}

/// Return the item Store path for a source item universe.
pub fn items_path(ws: &Workspace, source: &SourceConfig) -> PathBuf {
    store_root(ws).join(format!("items-{}.jsonl", source_slug(source)))
}

/// Return the action Store path for one configured source view/action plan.
pub fn actions_path(ws: &Workspace, source: &SourceConfig) -> PathBuf {
    store_root(ws).join(format!(
        "actions-{}-{}.jsonl",
        source_slug(source),
        source_hash(source)
    ))
}

/// Return the stable, readable item-universe identity for one source.
pub fn source_slug(source: &SourceConfig) -> String {
    let (kind, identity) = match &source.source {
        SourceKind::Jira { site, .. } => ("jira", normalize_site(site)),
        SourceKind::Qmd { collections, .. } => {
            let mut collections = collections.clone();
            collections.sort();
            ("qmd", collections.join(","))
        }
        SourceKind::Github { .. } => ("github", "github.com".to_string()),
    };
    format!("{kind}-{}-{}", slugify(&identity), short_hash(&identity))
}

/// Return the stable configured-source identity for action logs.
pub fn source_hash(source: &SourceConfig) -> String {
    short_hash(&serde_json::to_string(source).unwrap())
}

fn normalize_site(site: &str) -> String {
    site.trim()
        .trim_end_matches('/')
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .to_ascii_lowercase()
}

fn slugify(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

/// Expand a configured path into a filesystem path.
pub fn expand_path(s: &str) -> PathBuf {
    PathBuf::from(expand_vars(s))
}

/// Expand leading `~/`, `$VAR`, and `${VAR}` in trusted local config strings.
pub fn expand_vars(s: &str) -> String {
    let mut out = if let Some(rest) = s.strip_prefix("~/") {
        home_dir().join(rest).display().to_string()
    } else {
        s.to_string()
    };
    for (k, v) in env::vars() {
        out = out
            .replace(&format!("${k}"), &v)
            .replace(&format!("${{{k}}}"), &v);
    }
    out
}

/// Hash a JSON value as stable hex for action identity.
pub fn hash_json(v: &Value) -> String {
    let mut h = Sha256::new();
    h.update(serde_json::to_vec(v).unwrap());
    hex::encode(h.finalize())
}

/// Return a short stable hash for workspace ids derived from explicit paths.
pub fn short_hash(s: &str) -> String {
    hex::encode(Sha256::digest(s.as_bytes()))[..12].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentboard_core::model::{
        GithubCredentialConfig, GithubSourceMode, SourceConfig, SourceKind,
    };
    use std::collections::BTreeMap;

    #[test]
    fn qmd_source_requires_collections_and_query() {
        let config = WorkspaceConfig {
            sources: vec![SourceConfig {
                id: "local".into(),
                source: SourceKind::Qmd {
                    collections: vec![],
                    query: "ready".into(),
                    limit: 10,
                    map: Default::default(),
                },
                actions: vec![],
            }],
        };
        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn jira_source_requires_site_and_jql() {
        let config = WorkspaceConfig {
            sources: vec![SourceConfig {
                id: "jira".into(),
                source: SourceKind::Jira {
                    site: "".into(),
                    email_env: "JIRA_EMAIL".into(),
                    token_env: "JIRA_API_TOKEN".into(),
                    credentials: None,
                    jql: "project = AB".into(),
                    limit: 50,
                    fields: vec![],
                    field_map: Default::default(),
                    status_map: Default::default(),
                },
                actions: vec![],
            }],
        };
        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn jira_site_controls_item_store_identity() {
        let first = jira_source("https://team-a.atlassian.net/", "project = AB");
        let same_site = jira_source("https://team-a.atlassian.net", "assignee = currentUser()");
        let other_site = jira_source("https://team-b.atlassian.net", "project = AB");

        assert_eq!(source_slug(&first), source_slug(&same_site));
        assert_ne!(source_slug(&first), source_slug(&other_site));
    }

    #[test]
    fn source_hash_tracks_configured_source_view() {
        let first = jira_source("https://team-a.atlassian.net", "project = AB");
        let changed_jql = jira_source("https://team-a.atlassian.net", "assignee = currentUser()");

        assert_ne!(source_hash(&first), source_hash(&changed_jql));
    }

    #[test]
    fn store_paths_use_slug_and_hash() {
        let source = jira_source("https://team-a.atlassian.net", "project = AB");
        let ws = Workspace {
            id: "work".into(),
            path: "work.toml".into(),
            config: WorkspaceConfig {
                sources: vec![source.clone()],
            },
        };

        let items = items_path(&ws, &source)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let actions = actions_path(&ws, &source)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();

        assert!(items.starts_with("items-jira-team-a-atlassian-net-"));
        assert!(actions.starts_with("actions-jira-team-a-atlassian-net-"));
        assert!(actions.ends_with(&format!("-{}.jsonl", source_hash(&source))));
    }

    #[test]
    fn github_issue_source_requires_query_helper_and_limit() {
        let mut source = github_source("repo:zenobi-us/agentboard is:open");
        assert!(validate_config(&WorkspaceConfig {
            sources: vec![source.clone()]
        })
        .is_ok());

        source.source = SourceKind::Github {
            mode: GithubSourceMode::Issue,
            query: "".into(),
            credentials: GithubCredentialConfig {
                helper: "gh auth token".into(),
            },
            limit: 50,
            field_map: Default::default(),
            status_map: Default::default(),
        };
        assert!(validate_config(&WorkspaceConfig {
            sources: vec![source]
        })
        .is_err());
    }

    #[test]
    fn github_sources_share_item_store_identity() {
        let first = github_source("repo:zenobi-us/agentboard is:open");
        let second = github_source("repo:zenobi-us/agentboard label:ready");

        assert_eq!(source_slug(&first), source_slug(&second));
        assert!(source_slug(&first).starts_with("github-github-com-"));
    }

    #[test]
    fn github_status_map_must_be_explicit_in_config() {
        let missing = r#"
            [[sources]]
            id = "github"

            [sources.source]
            kind = "github"
            mode = "issue"
            query = "repo:zenobi-us/agentboard is:open"

            [sources.source.credentials]
            helper = "gh auth token"
        "#;
        assert!(toml::from_str::<WorkspaceConfig>(missing).is_err());

        let explicit_empty = r#"
            [[sources]]
            id = "github"

            [sources.source]
            kind = "github"
            mode = "issue"
            query = "repo:zenobi-us/agentboard is:open"
            status_map = {}

            [sources.source.credentials]
            helper = "gh auth token"
        "#;
        let config = toml::from_str::<WorkspaceConfig>(explicit_empty).unwrap();
        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn github_status_map_rejects_empty_entries() {
        let mut source = github_source("repo:zenobi-us/agentboard is:open");
        if let SourceKind::Github { status_map, .. } = &mut source.source {
            *status_map = BTreeMap::from([("ready".into(), "".into())]);
        }

        assert!(validate_config(&WorkspaceConfig {
            sources: vec![source]
        })
        .is_err());
    }

    fn jira_source(site: &str, jql: &str) -> SourceConfig {
        SourceConfig {
            id: "jira".into(),
            source: SourceKind::Jira {
                site: site.into(),
                email_env: "JIRA_EMAIL".into(),
                token_env: "JIRA_API_TOKEN".into(),
                credentials: None,
                jql: jql.into(),
                limit: 50,
                fields: vec![],
                field_map: Default::default(),
                status_map: Default::default(),
            },
            actions: vec![],
        }
    }

    fn github_source(query: &str) -> SourceConfig {
        SourceConfig {
            id: "github".into(),
            source: SourceKind::Github {
                mode: GithubSourceMode::Issue,
                query: query.into(),
                credentials: GithubCredentialConfig {
                    helper: "gh auth token".into(),
                },
                limit: 50,
                field_map: Default::default(),
                status_map: [("ready".to_string(), "ready".to_string())].into(),
            },
            actions: vec![],
        }
    }
}

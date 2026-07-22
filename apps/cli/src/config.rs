//! Workspace discovery, Registry-driven loading, and stable Store identities.
//!
//! Filesystem selection stays separate from text parsing so config behavior can
//! be tested without user directories and reused by every command consistently.

use std::{collections::HashSet, env, fs, path::PathBuf};

use agentboard_core::{
    model::{ActionConfig, GithubSourceMode, SourceConfig, SourceKind, Workspace, WorkspaceConfig},
    registry::{BuiltSource, Registry, SourceEnvelope, WorkspaceEnvelope},
};
use anyhow::{bail, Context, Result};
use directories::BaseDirs;
use serde_json::Value;
use sha2::{Digest, Sha256};

/// List named workspace config files from the user config directory.
pub fn list_workspaces() -> Result<Vec<String>> {
    let dir = config_home().join("agentboard");
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(error) => {
            return Err(error).with_context(|| format!("read workspaces {}", dir.display()))
        }
    };
    let mut names = Vec::new();
    for entry in entries {
        let path = entry?.path();
        if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("toml") {
            if let Some(name) = path.file_stem().and_then(|stem| stem.to_str()) {
                names.push(name.to_owned());
            }
        }
    }
    names.sort();
    Ok(names)
}

/// Return the config path for a named Workspace.
pub fn named_workspace_path(name: &str) -> PathBuf {
    config_home()
        .join("agentboard")
        .join(format!("{name}.toml"))
}

/// Validate a named Workspace identifier.
pub fn validate_workspace_name(name: &str) -> Result<()> {
    if name.is_empty()
        || !name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        bail!("workspace name must contain only letters, numbers, '-' or '_'");
    }
    Ok(())
}

/// Create an empty named Workspace without overwriting an existing config.
pub fn init_workspace(name: &str) -> Result<PathBuf> {
    validate_workspace_name(name)?;
    let path = named_workspace_path(name);
    if path.exists() {
        bail!("workspace already exists: {}", path.display());
    }
    fs::create_dir_all(path.parent().unwrap())?;
    fs::write(&path, "sources = []\n")?;
    Ok(path)
}

/// Carries Registry-loaded Sources before filesystem identity is attached.
///
/// Keeping this seam text-only makes loader behavior testable without XDG paths,
/// and lets schema and loading share the same caller-supplied Registry.
pub struct ParsedWorkspace {
    pub configured: WorkspaceEnvelope,
    pub sources: Vec<BuiltSource>,
}

/// Parses the stable Workspace envelope, then delegates typed config to registrations.
///
/// Source runtimes are built exactly once here. Actions are only validated because
/// their final typed inputs do not exist until templates render for an Item.
pub fn parse_workspace(text: &str, registry: &Registry) -> Result<ParsedWorkspace> {
    let envelope: WorkspaceEnvelope = toml::from_str(text).context("parse workspace TOML")?;
    let mut ids = HashSet::new();
    let mut configured_sources = Vec::with_capacity(envelope.sources.len());
    let mut built_sources = Vec::with_capacity(envelope.sources.len());

    for source in envelope.sources {
        if source.id.trim().is_empty() {
            bail!("source id cannot be empty");
        }
        if !ids.insert(source.id.clone()) {
            bail!("duplicate source id {}", source.id);
        }

        let kind = source.source.kind.clone();
        let built = registry
            .build_configured_source(&kind, source.source.config)
            .with_context(|| format!("source {} registration {kind}", source.id))?;

        for (index, action) in source.actions.iter().enumerate() {
            registry
                .validate_action(&action.uses, &action.inputs)
                .with_context(|| {
                    format!(
                        "source {} action {index} registration {}",
                        source.id, action.uses
                    )
                })?;
        }

        // Rebuild the serializable envelope from typed config so defaults and field
        // names exactly match the values that produced the runtime.
        configured_sources.push(agentboard_core::registry::ConfiguredSourceEnvelope {
            id: source.id,
            source: SourceEnvelope {
                kind,
                config: built.config().clone(),
            },
            actions: source.actions,
        });
        built_sources.push(built);
    }

    Ok(ParsedWorkspace {
        configured: WorkspaceEnvelope {
            sources: configured_sources,
        },
        sources: built_sources,
    })
}

/// Load a workspace by name or explicit TOML path through the process Registry.
///
/// When no input is supplied, load `.agentboard.toml` from the current
/// directory. Named workspaces resolve under the user config directory.
/// Explicit paths get a stable workspace id from the file stem plus canonical
/// path hash.
pub fn load_workspace(input: Option<&str>, registry: &Registry) -> Result<Workspace> {
    let (path, named_id) = resolve_workspace_input(input);
    let text =
        fs::read_to_string(&path).with_context(|| format!("read workspace {}", path.display()))?;
    let parsed = parse_workspace(&text, registry)?;
    // Issue #24 removes this compatibility conversion when runtime callers consume
    // `built_sources` and the registered configured view directly.
    let config: WorkspaceConfig = serde_json::from_value(serde_json::to_value(&parsed.configured)?)
        .context("convert registered Workspace to current runtime view")?;
    let id = if let Some(id) = named_id {
        id
    } else {
        let canon = fs::canonicalize(&path)?;
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("workspace");
        format!("{stem}-{}", short_hash(&canon.display().to_string()))
    };
    Ok(Workspace {
        id,
        path,
        config,
        built_sources: parsed.sources,
    })
}

/// Uses the same strict Registry loader for `doctor` so invalid config never reaches checks.
pub fn load_workspace_for_doctor(input: Option<&str>, registry: &Registry) -> Result<Workspace> {
    load_workspace(input, registry)
}

fn resolve_workspace_input(input: Option<&str>) -> (PathBuf, Option<String>) {
    match input {
        None => (PathBuf::from(".agentboard.toml"), None),
        Some(input) if input.ends_with(".toml") || input.contains('/') => {
            (expand_path(input), None)
        }
        Some(input) => (named_workspace_path(input), Some(input.to_string())),
    }
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
            validate_action(action)?;
        }
    }
    Ok(())
}

/// Validate one configured Action without rendering or executing it.
pub fn validate_action(action: &ActionConfig) -> Result<()> {
    match action.uses.as_str() {
        "agentboard/create-worktree" => require_inputs(action, &["repo", "root", "branch"]),
        "agentboard/run-cmd" => require_inputs(action, &["cmd"]),
        other if other.starts_with("agentboard/") => bail!("unknown built-in action {other}"),
        other => bail!("unknown action {other}"),
    }
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
    use agentboard_core::registry::{
        RuntimeResult, Source, SourceCollection, SourceContext, SourceDefinition, SourceFuture,
    };
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Detects accidental double construction at the public Registry/config seam.
    static SOURCE_BUILDS: AtomicUsize = AtomicUsize::new(0);

    #[derive(Deserialize, Serialize, JsonSchema)]
    #[serde(deny_unknown_fields)]
    struct CountingSourceConfig {
        value: String,
    }

    struct CountingSource;

    /// Counts construction so the loader cannot accidentally validate and build separately.
    struct CountingSourceDefinition;

    impl SourceDefinition for CountingSourceDefinition {
        const ID: &'static str = "counting";
        type Config = CountingSourceConfig;
        type Runtime = CountingSource;

        fn build(_config: Self::Config) -> RuntimeResult<Self::Runtime> {
            SOURCE_BUILDS.fetch_add(1, Ordering::SeqCst);
            Ok(CountingSource)
        }
    }

    impl Source for CountingSource {
        fn collect<'a>(&'a self, _context: &'a SourceContext<'a>) -> SourceFuture<'a> {
            Box::pin(async {
                Ok(SourceCollection {
                    items: vec![],
                    available: Some(0),
                    limit: 0,
                })
            })
        }

        fn item_bucket_identity(&self) -> String {
            "counting".into()
        }
    }

    /// Protects the registry/config seam: current TOML shape builds one runtime and keeps its view.
    #[test]
    fn registry_parses_workspace_and_builds_each_source_once() {
        SOURCE_BUILDS.store(0, Ordering::SeqCst);
        let mut registry = agentboard_core::registry::Registry::new();
        registry.add_source::<CountingSourceDefinition>().unwrap();

        let loaded = parse_workspace(
            r#"
                [[sources]]
                id = "local"

                [sources.source]
                kind = "counting"
                value = "configured"
            "#,
            &registry,
        )
        .unwrap();

        assert_eq!(SOURCE_BUILDS.load(Ordering::SeqCst), 1);
        assert_eq!(loaded.sources.len(), 1);
        assert_eq!(loaded.sources[0].registration_id(), "counting");
        assert_eq!(loaded.sources[0].config()["value"], "configured");
        assert_eq!(loaded.configured.sources[0].id, "local");
        assert_eq!(loaded.configured.sources[0].source.kind, "counting");
    }

    /// Keeps loader failures actionable across the generic Registry boundary.
    #[test]
    fn registry_load_errors_include_location_registration_and_underlying_error() {
        let registry = crate::cli::register_builtins().unwrap();
        let unknown_source = parse_workspace(
            r#"
                [[sources]]
                id = "local"
                [sources.source]
                kind = "missing"
            "#,
            &registry,
        )
        .err()
        .expect("unknown Source should fail");
        let invalid_source = parse_workspace(
            r#"
                [[sources]]
                id = "local"
                [sources.source]
                kind = "qmd"
                collections = ["tasks"]
                query = "ready"
                extra = true
            "#,
            &registry,
        )
        .err()
        .expect("invalid Source config should fail");
        let invalid_action = parse_workspace(
            r#"
                [[sources]]
                id = "local"
                [sources.source]
                kind = "qmd"
                collections = ["tasks"]
                query = "ready"
                [[sources.actions]]
                uses = "agentboard/run-cmd"
            "#,
            &registry,
        )
        .err()
        .expect("invalid Action config should fail");

        assert!(format!("{unknown_source:#}")
            .contains("source local registration missing: unknown source registration missing"));
        assert!(format!("{invalid_source:#}").contains(
            "source local registration qmd: invalid config for source qmd: unknown field `extra`"
        ));
        assert!(format!("{invalid_action:#}").contains(
            "source local action 0 registration agentboard/run-cmd: invalid config for action agentboard/run-cmd: missing field `cmd`"
        ));
    }

    #[test]
    fn omitted_workspace_uses_cwd_agentboard_file() {
        assert_eq!(
            resolve_workspace_input(None),
            (PathBuf::from(".agentboard.toml"), None)
        );
    }

    #[test]
    fn supplied_workspace_keeps_existing_resolution_rules() {
        assert_eq!(
            resolve_workspace_input(Some("./work.toml")),
            (PathBuf::from("./work.toml"), None)
        );
        assert_eq!(
            resolve_workspace_input(Some("work")).1,
            Some("work".to_string())
        );
    }

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
            built_sources: vec![],
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

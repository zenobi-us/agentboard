//! Workspace discovery, Registry-driven loading, and stable Store identities.
//!
//! Filesystem selection stays separate from text parsing so config behavior can
//! be tested without user directories and reused by every command consistently.

use std::{collections::HashSet, env, fs, path::PathBuf};

use agentboard_core::{
    model::{Workspace, WorkspaceSource},
    registry::{ConfiguredSourceEnvelope, Registry, SourceEnvelope, WorkspaceEnvelope},
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

/// Carries configured Sources already paired to their single constructed runtime.
pub struct ParsedWorkspace {
    pub sources: Vec<WorkspaceSource>,
}

/// Parses the stable Workspace envelope, then delegates typed config to registrations.
///
/// Source runtimes are built exactly once here. Actions are only validated because
/// their final typed inputs do not exist until templates render for an Item.
pub fn parse_workspace(text: &str, registry: &Registry) -> Result<ParsedWorkspace> {
    let envelope: WorkspaceEnvelope = toml::from_str(text).context("parse workspace TOML")?;
    let mut ids = HashSet::new();
    let mut sources = Vec::with_capacity(envelope.sources.len());

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

        sources.push(WorkspaceSource {
            configured: ConfiguredSourceEnvelope {
                id: source.id,
                source: SourceEnvelope {
                    kind,
                    config: built.config().clone(),
                },
                actions: source.actions,
            },
            built,
        });
    }

    Ok(ParsedWorkspace { sources })
}

/// Load a workspace by name or explicit TOML path through the process Registry.
pub fn load_workspace(input: Option<&str>, registry: &Registry) -> Result<Workspace> {
    let (path, named_id) = resolve_workspace_input(input);
    let text =
        fs::read_to_string(&path).with_context(|| format!("read workspace {}", path.display()))?;
    let parsed = parse_workspace(&text, registry)?;
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
        sources: parsed.sources,
    })
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
pub fn items_path(ws: &Workspace, source: &WorkspaceSource) -> PathBuf {
    store_root(ws).join(format!("items-{}.jsonl", source_slug(source)))
}

/// Return the action Store path for one configured source view/action plan.
pub fn actions_path(ws: &Workspace, source: &WorkspaceSource) -> PathBuf {
    store_root(ws).join(format!(
        "actions-{}-{}.jsonl",
        source_slug(source),
        source_hash(source)
    ))
}

/// Return the stable, readable item-universe identity for one registered Source.
pub fn source_slug(source: &WorkspaceSource) -> String {
    let kind = source.built.registration_id();
    let identity = source.built.runtime().item_bucket_identity();
    format!("{kind}-{}-{}", slugify(&identity), short_hash(&identity))
}

/// Return the stable configured-source identity for action logs.
pub fn source_hash(source: &WorkspaceSource) -> String {
    short_hash(&configured_source_json(source))
}

// Preserve the pre-Registry SourceConfig serialization byte-for-byte. Typed config
// JSON retains each registration's field order; CLI still owns the outer identity.
fn configured_source_json(source: &WorkspaceSource) -> String {
    debug_assert_eq!(
        source.configured.source.kind,
        source.built.registration_id()
    );
    let config = source.built.config_json();
    let fields = config
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
        .expect("registered Source config must serialize as an object");
    let kind = serde_json::to_string(source.built.registration_id()).unwrap();
    let source_json = if fields.is_empty() {
        format!("{{\"kind\":{kind}}}")
    } else {
        format!("{{\"kind\":{kind},{fields}}}")
    };
    format!(
        "{{\"id\":{},\"source\":{},\"actions\":{}}}",
        serde_json::to_string(&source.configured.id).unwrap(),
        source_json,
        serde_json::to_string(&source.configured.actions).unwrap()
    )
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
    use agentboard_core::registry::{
        RuntimeResult, Source, SourceCollection, SourceContext, SourceDefinition, SourceFuture,
    };
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static SOURCE_BUILDS: AtomicUsize = AtomicUsize::new(0);

    #[derive(Deserialize, Serialize, JsonSchema)]
    #[serde(deny_unknown_fields)]
    struct CountingSourceConfig {
        value: String,
    }

    struct CountingSource {
        value: String,
    }

    struct CountingSourceDefinition;

    impl SourceDefinition for CountingSourceDefinition {
        const ID: &'static str = "counting";
        type Config = CountingSourceConfig;
        type Runtime = CountingSource;

        fn build(config: Self::Config) -> RuntimeResult<Self::Runtime> {
            SOURCE_BUILDS.fetch_add(1, Ordering::SeqCst);
            Ok(CountingSource {
                value: config.value,
            })
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
            self.value.clone()
        }
    }

    #[test]
    fn registry_builds_once_and_pairs_sources_in_workspace_order() {
        SOURCE_BUILDS.store(0, Ordering::SeqCst);
        let mut registry = Registry::new();
        registry.add_source::<CountingSourceDefinition>().unwrap();

        let loaded = parse_workspace(
            r#"
                [[sources]]
                id = "first"
                [sources.source]
                kind = "counting"
                value = "bucket-a"

                [[sources]]
                id = "second"
                [sources.source]
                kind = "counting"
                value = "bucket-b"
            "#,
            &registry,
        )
        .unwrap();

        assert_eq!(SOURCE_BUILDS.load(Ordering::SeqCst), 2);
        assert_eq!(loaded.sources[0].configured.id, "first");
        assert_eq!(
            loaded.sources[0].built.runtime().item_bucket_identity(),
            "bucket-a"
        );
        assert_eq!(loaded.sources[1].configured.id, "second");
        assert_eq!(
            loaded.sources[1].built.runtime().item_bucket_identity(),
            "bucket-b"
        );
    }

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
    fn registered_cutover_preserves_builtin_store_paths_byte_for_byte() {
        let registry = crate::cli::register_builtins().unwrap();
        let parsed = parse_workspace(
            r#"
                [[sources]]
                id = "notes"
                [sources.source]
                kind = "qmd"
                collections = ["work", "ops"]
                query = "status:ready"
                [[sources.actions]]
                uses = "agentboard/run-cmd"
                [sources.actions.with]
                cmd = "echo {{ item.id }}"

                [[sources]]
                id = "jira"
                [sources.source]
                kind = "jira"
                site = "https://Team-A.atlassian.net/"
                jql = "project = AB"
                [[sources.actions]]
                uses = "agentboard/run-cmd"
                [sources.actions.with]
                cmd = "true"

                [[sources]]
                id = "github"
                [sources.source]
                kind = "github"
                mode = "issue"
                query = "repo:zenobi-us/agentboard is:open"
                status_map = { ready = "ready" }
                [sources.source.credentials]
                helper = "gh auth token"
                [[sources.actions]]
                uses = "agentboard/create-worktree"
                [sources.actions.with]
                branch = "item-{{ item.id }}"
                repo = "/repo"
                root = "/worktree"
            "#,
            &registry,
        )
        .unwrap();
        let ws = Workspace {
            id: "work".into(),
            path: "work.toml".into(),
            sources: parsed.sources,
        };
        let cases = [
            (
                &ws.sources[0],
                "items-qmd-ops-work-689cc400d680.jsonl",
                "actions-qmd-ops-work-689cc400d680-97357d02e12c.jsonl",
            ),
            (
                &ws.sources[1],
                "items-jira-team-a-atlassian-net-104c2a00a65c.jsonl",
                "actions-jira-team-a-atlassian-net-104c2a00a65c-e38419b90306.jsonl",
            ),
            (
                &ws.sources[2],
                "items-github-github-com-3aeb00246038.jsonl",
                "actions-github-github-com-3aeb00246038-fd21721e81de.jsonl",
            ),
        ];
        for (source, expected_items, expected_actions) in cases {
            assert_eq!(
                items_path(&ws, source)
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap(),
                expected_items
            );
            assert_eq!(
                actions_path(&ws, source)
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap(),
                expected_actions
            );
        }
    }
}

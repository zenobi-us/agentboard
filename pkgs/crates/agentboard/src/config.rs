use std::{collections::HashSet, env, fs, path::PathBuf};

use anyhow::{bail, Context, Result};
use directories::BaseDirs;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{
    model::{ActionConfig, SourceKind, Workspace, WorkspaceConfig},
    query::parse_query,
};

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

pub fn validate_config(config: &WorkspaceConfig) -> Result<()> {
    let mut ids = HashSet::new();
    for src in &config.sources {
        if !ids.insert(&src.id) {
            bail!("duplicate source id {}", src.id);
        }
        if src.id.trim().is_empty() {
            bail!("source id cannot be empty");
        }
        if let Some(q) = &src.query {
            parse_query(q).with_context(|| format!("invalid query for source {}", src.id))?;
        }
        match &src.source {
            SourceKind::Markdown { .. } => {}
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

pub fn config_home() -> PathBuf {
    BaseDirs::new()
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".config"))
}

pub fn data_home() -> PathBuf {
    BaseDirs::new()
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".local/share"))
}

pub fn home_dir() -> PathBuf {
    BaseDirs::new()
        .map(|d| d.home_dir().to_path_buf())
        .or_else(|| env::var_os("HOME").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn store_root(ws: &Workspace) -> PathBuf {
    data_home().join("agentboard").join(&ws.id)
}

pub fn source_dir(ws: &Workspace, source_id: &str) -> PathBuf {
    store_root(ws).join("sources").join(source_id)
}

pub fn expand_path(s: &str) -> PathBuf {
    PathBuf::from(expand_vars(s))
}

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

pub fn hash_json(v: &Value) -> String {
    let mut h = Sha256::new();
    h.update(serde_json::to_vec(v).unwrap());
    hex::encode(h.finalize())
}

pub fn short_hash(s: &str) -> String {
    hex::encode(Sha256::digest(s.as_bytes()))[..12].to_string()
}

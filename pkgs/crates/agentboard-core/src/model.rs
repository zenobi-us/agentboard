//! Shared serializable domain records used across CLI, Source, and Action crates.

use std::{collections::BTreeMap, path::PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::registry::{BuiltSource, ConfiguredSourceEnvelope};

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FieldMap {
    pub id: Option<String>,
    pub title: Option<String>,
    pub status: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActionConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub uses: String,
    #[serde(default, rename = "with")]
    pub inputs: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub reference_id: String,
    pub title: String,
    pub status: String,
    pub url: String,
    pub source_id: String,
    pub source_kind: String,
    pub raw: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionAttempt {
    pub ts: String,
    pub source_id: String,
    pub item_id: String,
    pub source_action_index: usize,
    pub uses: String,
    pub rendered_action_hash: String,
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub message: Option<String>,
}

/// Keeps one configured Source inseparable from the runtime built from it.
#[derive(Clone)]
pub struct WorkspaceSource {
    pub configured: ConfiguredSourceEnvelope,
    pub built: BuiltSource,
}

/// Loaded Workspace with each configured Source paired to its registered runtime.
#[derive(Clone)]
pub struct Workspace {
    pub id: String,
    pub path: PathBuf,
    pub sources: Vec<WorkspaceSource>,
}

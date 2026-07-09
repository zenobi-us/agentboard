use std::{collections::BTreeMap, path::PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceConfig {
    pub sources: Vec<SourceConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SourceConfig {
    pub id: String,
    pub source: SourceKind,
    #[serde(default)]
    pub actions: Vec<ActionConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum SourceKind {
    Qmd {
        collections: Vec<String>,
        query: String,
        #[serde(default = "default_source_limit")]
        limit: usize,
        #[serde(default)]
        map: FieldMap,
    },
    Jira {
        site: String,
        #[serde(default = "default_jira_email_env")]
        email_env: String,
        #[serde(default = "default_jira_token_env")]
        token_env: String,
        #[serde(default)]
        credentials: Option<JiraCredentialConfig>,
        jql: String,
        #[serde(default = "default_source_limit")]
        limit: usize,
        #[serde(default)]
        fields: Vec<String>,
        #[serde(default)]
        field_map: FieldMap,
        #[serde(default)]
        status_map: BTreeMap<String, String>,
    },
    Github {
        mode: GithubSourceMode,
        query: String,
        credentials: GithubCredentialConfig,
        #[serde(default = "default_source_limit")]
        limit: usize,
        #[serde(default)]
        field_map: FieldMap,
        status_map: BTreeMap<String, String>,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum GithubSourceMode {
    Issue,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct JiraCredentialConfig {
    pub helper: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GithubCredentialConfig {
    pub helper: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FieldMap {
    pub id: Option<String>,
    pub title: Option<String>,
    pub status: Option<String>,
    pub url: Option<String>,
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

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActionConfig {
    pub uses: String,
    #[serde(default, rename = "with")]
    pub inputs: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
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

#[derive(Debug, Clone)]
pub struct Workspace {
    pub id: String,
    pub path: PathBuf,
    pub config: WorkspaceConfig,
}

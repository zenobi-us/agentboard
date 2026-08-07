//! Shared serializable domain records used across CLI, Source, and Action crates.

use std::{collections::BTreeMap, fmt, path::PathBuf};

use schemars::JsonSchema;
use serde::{de::Error as _, Deserialize, Serialize};
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

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActionOutcome {
    Success,
    Failure,
    Cancelled,
}

impl fmt::Display for ActionOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Success => "success",
            Self::Failure => "failure",
            Self::Cancelled => "cancelled",
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ActionAttempt {
    pub ts: String,
    pub source_id: String,
    pub item_id: String,
    pub source_action_index: usize,
    pub uses: String,
    pub rendered_action_hash: String,
    pub outcome: ActionOutcome,
    pub stdout: String,
    pub stderr: String,
    pub message: Option<String>,
}

impl<'de> Deserialize<'de> for ActionAttempt {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct StoredActionAttempt {
            ts: String,
            source_id: String,
            item_id: String,
            source_action_index: usize,
            uses: String,
            rendered_action_hash: String,
            outcome: Option<ActionOutcome>,
            success: Option<bool>,
            stdout: String,
            stderr: String,
            message: Option<String>,
        }

        let stored = StoredActionAttempt::deserialize(deserializer)?;
        let outcome = match (stored.outcome, stored.success) {
            (Some(outcome), _) => outcome,
            (None, Some(success)) => {
                if success {
                    ActionOutcome::Success
                } else {
                    ActionOutcome::Failure
                }
            }
            (None, None) => return Err(D::Error::missing_field("outcome")),
        };
        Ok(Self {
            ts: stored.ts,
            source_id: stored.source_id,
            item_id: stored.item_id,
            source_action_index: stored.source_action_index,
            uses: stored.uses,
            rendered_action_hash: stored.rendered_action_hash,
            outcome,
            stdout: stored.stdout,
            stderr: stored.stderr,
            message: stored.message,
        })
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_legacy_success_boolean_and_writes_explicit_outcome() {
        let attempt: ActionAttempt = serde_json::from_str(
            r#"{"ts":"2026-01-01T00:00:00Z","source_id":"source","item_id":"item","source_action_index":0,"uses":"action","rendered_action_hash":"hash","success":true,"stdout":"out","stderr":"","message":null}"#,
        )
        .unwrap();
        assert_eq!(attempt.outcome, ActionOutcome::Success);
        let stored = serde_json::to_value(attempt).unwrap();
        assert_eq!(stored["outcome"], "success");
        assert!(stored.get("success").is_none());
    }

    #[test]
    fn reads_legacy_failure_boolean() {
        let attempt: ActionAttempt = serde_json::from_str(
            r#"{"ts":"2026-01-01T00:00:00Z","source_id":"source","item_id":"item","source_action_index":0,"uses":"action","rendered_action_hash":"hash","success":false,"stdout":"","stderr":"error","message":"failed"}"#,
        )
        .unwrap();
        assert_eq!(attempt.outcome, ActionOutcome::Failure);
        assert_eq!(serde_json::to_value(attempt).unwrap()["outcome"], "failure");
    }

    #[test]
    fn reads_explicit_success_outcome() {
        let attempt: ActionAttempt = serde_json::from_str(
            r#"{"ts":"2026-01-01T00:00:00Z","source_id":"source","item_id":"item","source_action_index":0,"uses":"action","rendered_action_hash":"hash","outcome":"success","stdout":"","stderr":"","message":null}"#,
        )
        .unwrap();
        assert_eq!(attempt.outcome, ActionOutcome::Success);
    }

    #[test]
    fn reads_explicit_failure_outcome() {
        let attempt: ActionAttempt = serde_json::from_str(
            r#"{"ts":"2026-01-01T00:00:00Z","source_id":"source","item_id":"item","source_action_index":0,"uses":"action","rendered_action_hash":"hash","outcome":"failure","stdout":"","stderr":"error","message":"failed"}"#,
        )
        .unwrap();
        assert_eq!(attempt.outcome, ActionOutcome::Failure);
        assert!(serde_json::to_value(attempt)
            .unwrap()
            .get("success")
            .is_none());
    }

    #[test]
    fn reads_explicit_cancelled_outcome() {
        let attempt: ActionAttempt = serde_json::from_str(
            r#"{"ts":"2026-01-01T00:00:00Z","source_id":"source","item_id":"item","source_action_index":0,"uses":"action","rendered_action_hash":"hash","outcome":"cancelled","stdout":"partial","stderr":"","message":"cancelled"}"#,
        )
        .unwrap();
        assert_eq!(attempt.outcome, ActionOutcome::Cancelled);
    }

    #[test]
    fn rejects_attempt_without_outcome_or_legacy_success() {
        let error = serde_json::from_str::<ActionAttempt>(
            r#"{"ts":"2026-01-01T00:00:00Z","source_id":"source","item_id":"item","source_action_index":0,"uses":"action","rendered_action_hash":"hash","stdout":"","stderr":"","message":null}"#,
        )
        .unwrap_err();
        assert!(error.to_string().contains("outcome"));
    }

    #[test]
    fn cancelled_attempt_is_not_successful() {
        let attempt = ActionAttempt {
            ts: "2026-01-01T00:00:00Z".into(),
            source_id: "source".into(),
            item_id: "item".into(),
            source_action_index: 0,
            uses: "action".into(),
            rendered_action_hash: "hash".into(),
            outcome: ActionOutcome::Cancelled,
            stdout: "partial".into(),
            stderr: String::new(),
            message: Some("cancelled".into()),
        };
        assert_eq!(
            serde_json::to_value(attempt).unwrap()["outcome"],
            "cancelled"
        );
    }
}

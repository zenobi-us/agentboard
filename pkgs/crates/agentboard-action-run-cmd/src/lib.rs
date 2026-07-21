use std::{
    collections::BTreeMap,
    process::{Command as ProcessCommand, Stdio},
};

use anyhow::{anyhow, bail, Result};
use schemars::JsonSchema;
use serde::Deserialize;

use agentboard_core::{
    cap,
    model::{Item, SourceConfig, Workspace},
    registry::{Action, ActionContext, ActionDefinition, RuntimeResult},
    ActionRun,
};

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunCmdConfig {
    pub cmd: String,
    pub cwd: Option<String>,
}

pub struct RunCmdDefinition;

pub struct RunCmdAction {
    config: RunCmdConfig,
}

impl ActionDefinition for RunCmdDefinition {
    const ID: &'static str = "agentboard/run-cmd";
    type Config = RunCmdConfig;
    type Runtime = RunCmdAction;

    fn build(config: Self::Config) -> RuntimeResult<Self::Runtime> {
        Ok(RunCmdAction { config })
    }

    fn health_check() -> RuntimeResult<()> {
        check_command("sh", &["-c", ":"])
    }
}

impl Action for RunCmdAction {
    fn execute(&self, context: &ActionContext<'_>) -> RuntimeResult<ActionRun> {
        Ok(execute_command(&self.config, context))
    }
}

/// Temporary legacy bridge for the CLI cutover in issue #24.
pub fn run_cmd(
    ws: &Workspace,
    source: &SourceConfig,
    item: &Item,
    inputs: &BTreeMap<String, String>,
) -> Result<ActionRun> {
    let action = RunCmdDefinition::build(RunCmdConfig {
        cmd: inputs.get("cmd").unwrap().clone(),
        cwd: inputs.get("cwd").cloned(),
    })?;
    action.execute(&ActionContext {
        workspace_id: &ws.id,
        source_id: &source.id,
        item,
    })
}

fn execute_command(config: &RunCmdConfig, context: &ActionContext<'_>) -> ActionRun {
    let mut c = ProcessCommand::new("sh");
    c.arg("-c")
        .arg(&config.cmd)
        .stdin(Stdio::null())
        .env("AGENTBOARD_WORKSPACE_ID", context.workspace_id)
        .env("AGENTBOARD_SOURCE_ID", context.source_id)
        .env("AGENTBOARD_ITEM_ID", &context.item.id);
    if let Some(cwd) = &config.cwd {
        c.current_dir(cwd);
    }
    let out = match c.output() {
        Ok(output) => output,
        Err(error) => return failed_run(error.into()),
    };
    let success = out.status.success();
    ActionRun {
        success,
        stdout: cap(&out.stdout),
        stderr: cap(&out.stderr),
        message: (!success).then(|| format!("command exited with {}", out.status)),
    }
}

fn failed_run(error: anyhow::Error) -> ActionRun {
    ActionRun {
        success: false,
        stdout: String::new(),
        stderr: cap(format!("{error:#}").as_bytes()),
        message: Some(error.to_string()),
    }
}

fn check_command(command: &str, args: &[&str]) -> Result<()> {
    let status = ProcessCommand::new(command)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| anyhow!("required command {command} not found: {error}"))?;
    if !status.success() {
        bail!("required command {command} returned {status}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentboard_core::{
        model::Item,
        registry::{Action, ActionContext, ActionDefinition, Registry},
    };
    use serde_json::json;
    use tempfile::tempdir;

    fn item() -> Item {
        Item {
            id: "AB-22".into(),
            reference_id: "22".into(),
            title: "Typed actions".into(),
            status: "ready".into(),
            url: "https://example.test/items/22".into(),
            source_id: "issues".into(),
            source_kind: "github".into(),
            raw: json!({}),
        }
    }

    #[test]
    fn registers_run_cmd_config_schema() {
        let mut registry = Registry::new();
        registry.add_action::<RunCmdDefinition>().unwrap();

        let registration = registry.actions().next().unwrap();
        let schema = serde_json::to_value(registration.schema()).unwrap();

        assert_eq!(registration.id(), "agentboard/run-cmd");
        assert!(schema["properties"]["cmd"].is_object());
        assert!(schema["properties"]["cwd"].is_object());
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(schema["required"], json!(["cmd"]));
        assert!(registry
            .validate_action("agentboard/run-cmd", &BTreeMap::new())
            .is_err());
        assert!(registry
            .validate_action(
                "agentboard/run-cmd",
                &BTreeMap::from([
                    ("cmd".into(), "true".into()),
                    ("extra".into(), "value".into()),
                ]),
            )
            .is_err());

        let dir = tempdir().unwrap();
        let marker = dir.path().join("not-created");
        registry
            .build_action(
                "agentboard/run-cmd",
                BTreeMap::from([("cmd".into(), format!("touch {}", marker.display()))]),
            )
            .unwrap();
        assert!(!marker.exists());
    }

    #[test]
    fn executes_with_context_environment_and_cwd() {
        let dir = tempdir().unwrap();
        let action = RunCmdDefinition::build(RunCmdConfig {
            cmd: "printf '%s|%s|%s|%s' \"$AGENTBOARD_WORKSPACE_ID\" \"$AGENTBOARD_SOURCE_ID\" \"$AGENTBOARD_ITEM_ID\" \"$(pwd)\"".into(),
            cwd: Some(dir.path().display().to_string()),
        })
        .unwrap();
        let item = item();

        let run = action
            .execute(&ActionContext {
                workspace_id: "workspace",
                source_id: "issues",
                item: &item,
            })
            .unwrap();

        assert!(run.success);
        assert_eq!(
            run.stdout,
            format!("workspace|issues|AB-22|{}", dir.path().display())
        );
        assert!(run.stderr.is_empty());
        assert!(run.message.is_none());
    }

    #[test]
    fn normalizes_command_and_spawn_failures() {
        let item = item();
        let context = ActionContext {
            workspace_id: "workspace",
            source_id: "issues",
            item: &item,
        };
        let action = RunCmdDefinition::build(RunCmdConfig {
            cmd: "printf out; printf err >&2; exit 7".into(),
            cwd: None,
        })
        .unwrap();

        let run = action.execute(&context).unwrap();

        assert!(!run.success);
        assert_eq!(run.stdout, "out");
        assert_eq!(run.stderr, "err");
        assert!(run.message.unwrap().contains('7'));

        let action = RunCmdDefinition::build(RunCmdConfig {
            cmd: "true".into(),
            cwd: Some("/agentboard/path/that/does/not/exist".into()),
        })
        .unwrap();
        let run = action.execute(&context).unwrap();

        assert!(!run.success);
        assert!(!run.stderr.is_empty());
        assert!(run.message.is_some());
    }

    #[test]
    fn checks_required_shell_through_testable_boundary() {
        RunCmdDefinition::health_check().unwrap();

        let error = check_command("sh", &["-c", "exit 9"]).unwrap_err();
        assert!(error.to_string().contains("required command sh returned"));

        let error = check_command("agentboard-command-that-does-not-exist", &[]).unwrap_err();
        assert!(error
            .to_string()
            .contains("required command agentboard-command-that-does-not-exist not found"));
    }
}

use std::{
    process::{Command as ProcessCommand, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

use anyhow::{anyhow, bail, Result};
use command_group::CommandGroup;
use schemars::JsonSchema;
use serde::Deserialize;

use agentboard_core::{
    cap,
    model::ActionOutcome,
    registry::{Action, ActionContext, ActionDefinition, HealthCheckContext, RuntimeResult},
    ActionRun, STDOUT_LIMIT,
};

mod process;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunCmdConfig {
    pub cmd: String,
    pub cwd: Option<String>,
    pub healthcheck: Option<String>,
    #[serde(default = "default_healthcheck_interval")]
    #[schemars(default = "default_healthcheck_interval")]
    pub healthcheck_interval: String,
    #[serde(default = "default_healthcheck_timeout")]
    #[schemars(default = "default_healthcheck_timeout")]
    pub healthcheck_timeout: String,
}

pub struct RunCmdDefinition;

pub struct RunCmdAction {
    cmd: String,
    cwd: Option<String>,
    healthcheck: Option<LaunchHealthcheck>,
}

struct LaunchHealthcheck {
    command: String,
    interval: Duration,
    timeout: Duration,
}

fn default_healthcheck_interval() -> String {
    "1s".into()
}

fn default_healthcheck_timeout() -> String {
    "30s".into()
}

impl ActionDefinition for RunCmdDefinition {
    const ID: &'static str = "agentboard/run-cmd";
    type Config = RunCmdConfig;
    type Runtime = RunCmdAction;

    fn build(config: Self::Config) -> RuntimeResult<Self::Runtime> {
        let healthcheck = config
            .healthcheck
            .map(|command| -> Result<LaunchHealthcheck> {
                Ok(LaunchHealthcheck {
                    command,
                    interval: parse_duration("healthcheck_interval", &config.healthcheck_interval)?,
                    timeout: parse_duration("healthcheck_timeout", &config.healthcheck_timeout)?,
                })
            })
            .transpose()?;
        Ok(RunCmdAction {
            cmd: config.cmd,
            cwd: config.cwd,
            healthcheck,
        })
    }

    fn health_check(context: &HealthCheckContext<'_>) -> RuntimeResult<()> {
        check_command_with_cancellation("sh", &["-c", ":"], &context.cancellation)
    }
}

impl Action for RunCmdAction {
    fn execute(&self, context: &ActionContext<'_>) -> RuntimeResult<ActionRun> {
        Ok(execute_command(self, context))
    }
}

fn execute_command(action: &RunCmdAction, context: &ActionContext<'_>) -> ActionRun {
    if context.cancellation.is_cancelled() {
        return cancelled_run(anyhow!("action cancelled"));
    }
    let launch = match process::run(&action.cmd, action.cwd.as_deref(), context) {
        Ok(output) => output,
        Err(error) => {
            return if context.cancellation.is_cancelled() {
                cancelled_run(error)
            } else {
                failed_run(error)
            };
        }
    };
    if !launch.status.success() {
        return if context.cancellation.is_cancelled() {
            cancelled_output_run(launch)
        } else {
            output_run(launch)
        };
    }
    let Some(healthcheck) = &action.healthcheck else {
        return output_run(launch);
    };

    poll_healthcheck(healthcheck, action.cwd.as_deref(), context, launch)
}

fn poll_healthcheck(
    healthcheck: &LaunchHealthcheck,
    cwd: Option<&str>,
    context: &ActionContext<'_>,
    launch: Output,
) -> ActionRun {
    let started = Instant::now();
    let deadline = started + healthcheck.timeout;
    loop {
        let probe = match process::run_until(&healthcheck.command, cwd, context, deadline) {
            Ok(output) => output,
            Err(error) => {
                return if context.cancellation.is_cancelled() {
                    cancelled_output_run(launch)
                } else {
                    failed_run(error)
                };
            }
        };
        match probe {
            process::Run::Finished(probe) if probe.status.success() => return output_run(launch),
            process::Run::TimedOut(probe) => {
                return healthcheck_timeout_run(healthcheck.timeout, launch, probe)
            }
            process::Run::Finished(probe) => {
                if context.cancellation.is_cancelled() {
                    return cancelled_output_run(combined_output_run(launch, probe));
                }
                let elapsed = started.elapsed();
                if elapsed >= healthcheck.timeout {
                    return healthcheck_timeout_run(healthcheck.timeout, launch, probe);
                }
                let sleep_for = healthcheck.interval.min(healthcheck.timeout - elapsed);
                let sleep_started = Instant::now();
                while sleep_started.elapsed() < sleep_for {
                    if context.cancellation.is_cancelled() {
                        return cancelled_output_run(combined_output_run(launch, probe));
                    }
                    thread::sleep(
                        Duration::from_millis(5).min(sleep_for - sleep_started.elapsed()),
                    );
                }
                if started.elapsed() >= healthcheck.timeout {
                    return healthcheck_timeout_run(healthcheck.timeout, launch, probe);
                }
            }
        }
    }
}

fn output_run(output: Output) -> ActionRun {
    let success = output.status.success();
    ActionRun {
        outcome: if success {
            ActionOutcome::Success
        } else {
            ActionOutcome::Failure
        },
        stdout: cap(&output.stdout),
        stderr: cap(&output.stderr),
        message: (!success).then(|| format!("command exited with {}", output.status)),
    }
}

fn cancelled_output_run(output: Output) -> ActionRun {
    ActionRun {
        outcome: ActionOutcome::Cancelled,
        stdout: cap(&output.stdout),
        stderr: cap(&output.stderr),
        message: Some("action cancelled".into()),
    }
}

fn combined_output_run(launch: Output, probe: Output) -> Output {
    Output {
        status: probe.status,
        stdout: combined_output(&launch.stdout, &probe.stdout).into_bytes(),
        stderr: combined_output(&launch.stderr, &probe.stderr).into_bytes(),
    }
}

fn healthcheck_timeout_run(timeout: Duration, launch: Output, probe: Output) -> ActionRun {
    ActionRun {
        outcome: ActionOutcome::Failure,
        stdout: combined_output(&launch.stdout, &probe.stdout),
        stderr: combined_output(&launch.stderr, &probe.stderr),
        message: Some(format!(
            "healthcheck timed out after {}",
            humantime::format_duration(timeout)
        )),
    }
}

fn combined_output(launch: &[u8], probe: &[u8]) -> String {
    if probe.len() >= STDOUT_LIMIT {
        return cap(probe);
    }
    let separator_len = usize::from(!launch.is_empty() && !probe.is_empty());
    let launch_len = launch.len().min(STDOUT_LIMIT - probe.len() - separator_len);
    let mut output = Vec::with_capacity(launch_len + probe.len() + separator_len);
    output.extend_from_slice(&launch[..launch_len]);
    if separator_len == 1 {
        output.push(b'\n');
    }
    output.extend_from_slice(probe);
    cap(&output)
}

fn parse_duration(name: &str, value: &str) -> Result<Duration> {
    let duration = humantime::parse_duration(value)
        .map_err(|error| anyhow!("invalid {name} {value:?}: {error}"))?;
    if duration.is_zero() {
        bail!("invalid {name} {value:?}: duration must be greater than zero");
    }
    Ok(duration)
}

fn cancelled_run(error: anyhow::Error) -> ActionRun {
    ActionRun {
        outcome: ActionOutcome::Cancelled,
        stdout: String::new(),
        stderr: cap(format!("{error:#}").as_bytes()),
        message: Some(error.to_string()),
    }
}

fn failed_run(error: anyhow::Error) -> ActionRun {
    ActionRun {
        outcome: ActionOutcome::Failure,
        stdout: String::new(),
        stderr: cap(format!("{error:#}").as_bytes()),
        message: Some(error.to_string()),
    }
}

#[cfg(test)]
fn check_command(command: &str, args: &[&str]) -> Result<()> {
    check_command_with_cancellation(command, args, &agentboard_core::CancellationToken::new())
}

fn check_command_with_cancellation(
    command: &str,
    args: &[&str],
    cancellation: &agentboard_core::CancellationToken,
) -> Result<()> {
    if cancellation.is_cancelled() {
        bail!("required command {command} health check cancelled");
    }
    let mut child = ProcessCommand::new(command)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .group_spawn()
        .map_err(|error| anyhow!("required command {command} not found: {error}"))?;
    loop {
        if let Some(status) = child.try_wait()? {
            if !status.success() {
                bail!("required command {command} returned {status}");
            }
            return Ok(());
        }
        if cancellation.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            bail!("required command {command} health check cancelled");
        }
        thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentboard_core::{
        model::Item,
        registry::{Action, ActionContext, ActionDefinition, Registry},
        CancellationToken,
    };
    use serde_json::json;
    use std::collections::BTreeMap;
    use tempfile::tempdir;

    fn config(cmd: impl Into<String>) -> RunCmdConfig {
        RunCmdConfig {
            cmd: cmd.into(),
            cwd: None,
            healthcheck: None,
            healthcheck_interval: default_healthcheck_interval(),
            healthcheck_timeout: default_healthcheck_timeout(),
        }
    }

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

    fn run(action: &RunCmdAction) -> ActionRun {
        let item = item();
        action
            .execute(&ActionContext {
                workspace_id: "workspace",
                source_id: "issues",
                item: &item,
                cancellation: CancellationToken::new(),
            })
            .unwrap()
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
        assert!(schema["properties"]["healthcheck"].is_object());
        assert_eq!(
            schema["properties"]["healthcheck_interval"]["default"],
            "1s"
        );
        assert_eq!(
            schema["properties"]["healthcheck_timeout"]["default"],
            "30s"
        );
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

        for (name, value) in [
            ("healthcheck_interval", "not-a-duration"),
            ("healthcheck_timeout", "0s"),
        ] {
            let error = registry
                .build_action(
                    "agentboard/run-cmd",
                    BTreeMap::from([
                        ("cmd".into(), "true".into()),
                        ("healthcheck".into(), "true".into()),
                        (name.into(), value.into()),
                    ]),
                )
                .err()
                .unwrap();
            assert!(error.to_string().contains(name));
        }
    }

    #[test]
    fn executes_with_context_environment_and_cwd() {
        let dir = tempdir().unwrap();
        let mut config = config(
            "printf '%s|%s|%s|%s' \"$AGENTBOARD_WORKSPACE_ID\" \"$AGENTBOARD_SOURCE_ID\" \"$AGENTBOARD_ITEM_ID\" \"$(pwd)\"",
        );
        config.cwd = Some(dir.path().display().to_string());
        let action = RunCmdDefinition::build(config).unwrap();
        let run = run(&action);

        assert_eq!(run.outcome, ActionOutcome::Success);
        assert_eq!(
            run.stdout,
            format!("workspace|issues|AB-22|{}", dir.path().display())
        );
        assert!(run.stderr.is_empty());
        assert!(run.message.is_none());
    }

    #[test]
    fn normalizes_command_and_spawn_failures() {
        let action = RunCmdDefinition::build(config("printf out; printf err >&2; exit 7")).unwrap();

        let command_run = run(&action);

        assert_eq!(command_run.outcome, ActionOutcome::Failure);
        assert_eq!(command_run.stdout, "out");
        assert_eq!(command_run.stderr, "err");
        assert!(command_run.message.unwrap().contains('7'));

        let mut config = config("true");
        config.cwd = Some("/agentboard/path/that/does/not/exist".into());
        let action = RunCmdDefinition::build(config).unwrap();
        let spawn_run = run(&action);

        assert_eq!(spawn_run.outcome, ActionOutcome::Failure);
        assert!(spawn_run.stderr.contains("run shell command in cwd"));
        assert!(spawn_run
            .stderr
            .contains("/agentboard/path/that/does/not/exist"));
        assert!(spawn_run.message.is_some());
    }

    #[test]
    fn launch_failure_does_not_run_healthcheck() {
        let dir = tempdir().unwrap();
        let marker = dir.path().join("healthcheck-ran");
        let mut config = config("exit 7");
        config.healthcheck = Some(format!("touch {}", marker.display()));
        let action = RunCmdDefinition::build(config).unwrap();
        let run = run(&action);

        assert_eq!(run.outcome, ActionOutcome::Failure);
        assert!(!marker.exists());
    }

    #[test]
    fn healthcheck_succeeds_immediately_with_context_and_cwd() {
        let dir = tempdir().unwrap();
        let marker = dir.path().join("healthcheck-context");
        let mut config = config("true");
        config.cwd = Some(dir.path().display().to_string());
        config.healthcheck = Some(format!(
            "printf '%s|%s|%s|%s' \"$AGENTBOARD_WORKSPACE_ID\" \"$AGENTBOARD_SOURCE_ID\" \"$AGENTBOARD_ITEM_ID\" \"$(pwd)\" > {}",
            marker.display()
        ));
        let action = RunCmdDefinition::build(config).unwrap();
        let run = run(&action);

        assert_eq!(
            run.outcome,
            ActionOutcome::Success,
            "stdout={:?} stderr={:?} message={:?}",
            run.stdout,
            run.stderr,
            run.message
        );
        assert_eq!(
            std::fs::read_to_string(marker).unwrap(),
            format!("workspace|issues|AB-22|{}", dir.path().display())
        );
    }

    #[test]
    fn healthcheck_polls_until_success() {
        let dir = tempdir().unwrap();
        let attempts = dir.path().join("attempts");
        let mut config = config("true");
        config.healthcheck = Some(format!(
            "count=$(cat {0} 2>/dev/null || echo 0); count=$((count + 1)); echo $count > {0}; test $count -ge 3",
            attempts.display()
        ));
        config.healthcheck_interval = "10ms".into();
        config.healthcheck_timeout = "500ms".into();
        let action = RunCmdDefinition::build(config).unwrap();
        let run = run(&action);

        assert_eq!(run.outcome, ActionOutcome::Success);
        assert_eq!(std::fs::read_to_string(attempts).unwrap().trim(), "3");
    }

    #[test]
    fn healthcheck_timeout_reports_last_probe_output() {
        let mut config = config("printf launch-out; printf launch-err >&2");
        config.healthcheck = Some("printf probe-out; printf probe-err >&2; exit 1".into());
        config.healthcheck_interval = "10ms".into();
        config.healthcheck_timeout = "30ms".into();
        let action = RunCmdDefinition::build(config).unwrap();
        let run = run(&action);

        assert_eq!(run.outcome, ActionOutcome::Failure);
        assert!(run.stdout.contains("launch-out"));
        assert!(run.stdout.contains("probe-out"));
        assert!(run.stderr.contains("launch-err"));
        assert!(run.stderr.contains("probe-err"));
        assert!(run.message.unwrap().contains("timed out after 30ms"));
    }

    #[test]
    fn healthcheck_timeout_terminates_a_slow_probe() {
        let mut config = config("true");
        config.healthcheck = Some("printf started; sleep 1; printf late".into());
        config.healthcheck_interval = "10ms".into();
        config.healthcheck_timeout = "30ms".into();
        let action = RunCmdDefinition::build(config).unwrap();
        let started = Instant::now();
        let run = run(&action);

        assert_eq!(run.outcome, ActionOutcome::Failure);
        assert!(started.elapsed() < Duration::from_millis(500));
        assert!(run.stdout.contains("started"));
        assert!(!run.stdout.contains("late"));
        assert!(run.message.unwrap().contains("timed out after 30ms"));
    }

    #[test]
    fn cancellation_during_healthcheck_keeps_launch_and_probe_output() {
        let dir = tempdir().unwrap();
        let started = dir.path().join("healthcheck-started");
        let mut config = config("printf launch-out; printf launch-err >&2");
        config.healthcheck = Some(format!(
            "printf probe-out; printf probe-err >&2; touch {}; sleep 1; exit 1",
            started.display()
        ));
        let action = RunCmdDefinition::build(config).unwrap();
        let item = item();
        let cancellation = CancellationToken::new();
        let cancel = cancellation.clone();
        let handle = thread::spawn(move || {
            action
                .execute(&ActionContext {
                    workspace_id: "workspace",
                    source_id: "issues",
                    item: &item,
                    cancellation,
                })
                .unwrap()
        });
        for _ in 0..100 {
            if started.exists() {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert!(started.exists());
        cancel.cancel();

        let run = handle.join().unwrap();
        assert_eq!(run.outcome, ActionOutcome::Cancelled);
        assert_eq!(run.message.as_deref(), Some("action cancelled"));
        assert!(run.stdout.contains("launch-out"));
        assert!(run.stdout.contains("probe-out"));
        assert!(run.stderr.contains("launch-err"));
        assert!(run.stderr.contains("probe-err"));
    }

    #[test]
    fn cancellation_terminates_owned_command_group() {
        let action = RunCmdDefinition::build(config("sleep 5; printf late")).unwrap();
        let item = item();
        let cancellation = CancellationToken::new();
        let cancel = cancellation.clone();
        let started = Instant::now();
        let handle = thread::spawn(move || {
            action
                .execute(&ActionContext {
                    workspace_id: "workspace",
                    source_id: "issues",
                    item: &item,
                    cancellation,
                })
                .unwrap()
        });
        thread::sleep(Duration::from_millis(30));
        cancel.cancel();
        let run = handle.join().unwrap();
        assert!(started.elapsed() < Duration::from_secs(1));
        assert_eq!(run.outcome, ActionOutcome::Cancelled);
        assert!(!run.stdout.contains("late"));
    }

    #[test]
    fn already_cancelled_action_does_not_start_command() {
        let dir = tempdir().unwrap();
        let marker = dir.path().join("started");
        let action = RunCmdDefinition::build(config(format!(
            "touch {}; printf started",
            marker.display()
        )))
        .unwrap();
        let item = item();
        let cancellation = CancellationToken::new();
        cancellation.cancel();

        let run = action
            .execute(&ActionContext {
                workspace_id: "workspace",
                source_id: "issues",
                item: &item,
                cancellation,
            })
            .unwrap();

        assert_eq!(run.outcome, ActionOutcome::Cancelled);
        assert_eq!(run.message.as_deref(), Some("action cancelled"));
        assert!(!marker.exists());
    }

    #[test]
    fn cancellation_terminates_descendants_and_drains_output() {
        let dir = tempdir().unwrap();
        let started = dir.path().join("started");
        let descendant = dir.path().join("descendant-finished");
        let action = RunCmdDefinition::build(config(format!(
            "printf before; printf error-before >&2; touch {started}; (sleep 1; touch {descendant}) & wait",
            started = started.display(),
            descendant = descendant.display(),
        )))
        .unwrap();
        let item = item();
        let cancellation = CancellationToken::new();
        let cancel = cancellation.clone();
        let handle = thread::spawn(move || {
            action
                .execute(&ActionContext {
                    workspace_id: "workspace",
                    source_id: "issues",
                    item: &item,
                    cancellation,
                })
                .unwrap()
        });
        for _ in 0..100 {
            if started.exists() {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert!(started.exists());
        cancel.cancel();

        let run = handle.join().unwrap();
        assert_eq!(run.outcome, ActionOutcome::Cancelled);
        assert_eq!(run.message.as_deref(), Some("action cancelled"));
        assert_eq!(run.stdout, "before");
        assert_eq!(run.stderr, "error-before");
        thread::sleep(Duration::from_millis(100));
        assert!(!descendant.exists());
    }

    #[test]
    fn healthcheck_cleans_up_background_processes_before_timeout() {
        let mut config = config("true");
        config.healthcheck = Some("sleep 1 & printf probe; exit 1".into());
        config.healthcheck_interval = "10ms".into();
        config.healthcheck_timeout = "30ms".into();
        let action = RunCmdDefinition::build(config).unwrap();
        let started = Instant::now();
        let run = run(&action);

        assert_eq!(run.outcome, ActionOutcome::Failure);
        assert!(started.elapsed() < Duration::from_millis(500));
        assert!(run.stdout.contains("probe"));
        assert!(run.message.unwrap().contains("timed out after 30ms"));
    }

    #[test]
    fn checks_required_shell_through_testable_boundary() {
        RunCmdDefinition::health_check(&agentboard_core::registry::HealthCheckContext {
            source_id: "source",
            cancellation: CancellationToken::new(),
        })
        .unwrap();

        let error = check_command("sh", &["-c", "exit 9"]).unwrap_err();
        assert!(error.to_string().contains("required command sh returned"));

        let error = check_command("agentboard-command-that-does-not-exist", &[]).unwrap_err();
        assert!(error
            .to_string()
            .contains("required command agentboard-command-that-does-not-exist not found"));
    }
}

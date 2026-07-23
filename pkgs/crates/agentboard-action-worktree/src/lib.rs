use std::{
    path::Path,
    process::{Command as ProcessCommand, Stdio},
};

use anyhow::{anyhow, bail, Result};
use schemars::JsonSchema;
use serde::Deserialize;

use agentboard_core::{
    cap,
    registry::{Action, ActionContext, ActionDefinition, RuntimeResult},
    ActionRun,
};

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateWorktreeConfig {
    pub repo: String,
    pub root: String,
    pub branch: String,
}

pub struct CreateWorktreeDefinition;

pub struct CreateWorktreeAction {
    config: CreateWorktreeConfig,
}

impl ActionDefinition for CreateWorktreeDefinition {
    const ID: &'static str = "agentboard/create-worktree";
    type Config = CreateWorktreeConfig;
    type Runtime = CreateWorktreeAction;

    fn build(config: Self::Config) -> RuntimeResult<Self::Runtime> {
        Ok(CreateWorktreeAction { config })
    }

    fn health_check() -> RuntimeResult<()> {
        check_command("git", &["--version"])
    }
}

impl Action for CreateWorktreeAction {
    fn execute(&self, _context: &ActionContext<'_>) -> RuntimeResult<ActionRun> {
        Ok(execute_worktree(&self.config))
    }
}

fn execute_worktree(config: &CreateWorktreeConfig) -> ActionRun {
    if Path::new(&config.root).exists() {
        let out = ProcessCommand::new("git")
            .args(["-C", &config.root, "branch", "--show-current"])
            .output();
        let out = match out {
            Ok(output) => output,
            Err(error) => return failed_run(error.into()),
        };
        let current = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if out.status.success() && current == config.branch {
            return successful_run(format!("reused {}\n", config.root), String::new());
        }
        return failed_message(format!(
            "{} exists but is not worktree for branch {}",
            config.root, config.branch
        ));
    }
    let exists = ProcessCommand::new("git")
        .args(["-C", &config.repo, "rev-parse", "--verify", &config.branch])
        .output();
    let exists = match exists {
        Ok(output) => output.status.success(),
        Err(error) => return failed_run(error.into()),
    };
    let mut cmd = ProcessCommand::new("git");
    cmd.arg("-C").arg(&config.repo).arg("worktree").arg("add");
    if exists {
        cmd.arg(&config.root).arg(&config.branch);
    } else {
        cmd.arg("-b").arg(&config.branch).arg(&config.root);
    }
    let out = match cmd.output() {
        Ok(output) => output,
        Err(error) => return failed_run(error.into()),
    };
    if !out.status.success() {
        let message = format!("git worktree failed with {}", out.status);
        let stderr = cap(&out.stderr);
        return ActionRun {
            success: false,
            stdout: String::new(),
            stderr: if stderr.is_empty() {
                message.clone()
            } else {
                stderr
            },
            message: Some(message),
        };
    }
    successful_run(cap(&out.stdout), cap(&out.stderr))
}

fn successful_run(stdout: String, stderr: String) -> ActionRun {
    ActionRun {
        success: true,
        stdout,
        stderr,
        message: None,
    }
}

fn failed_message(message: String) -> ActionRun {
    ActionRun {
        success: false,
        stdout: String::new(),
        stderr: cap(message.as_bytes()),
        message: Some(message),
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
    use std::{collections::BTreeMap, fs};
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

    fn git(repo: &Path, args: &[&str]) {
        let status = ProcessCommand::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed with {status}");
    }

    fn init_repo(repo: &Path) {
        fs::create_dir(repo).unwrap();
        git(repo, &["init", "-q"]);
        git(repo, &["config", "user.email", "agentboard@example.test"]);
        git(repo, &["config", "user.name", "AgentBoard"]);
        fs::write(repo.join("README.md"), "test\n").unwrap();
        git(repo, &["add", "README.md"]);
        git(repo, &["commit", "-m", "initial"]);
    }

    fn config(repo: &Path, root: &Path, branch: &str) -> CreateWorktreeConfig {
        CreateWorktreeConfig {
            repo: repo.display().to_string(),
            root: root.display().to_string(),
            branch: branch.into(),
        }
    }

    #[test]
    fn registers_create_worktree_config_schema() {
        let mut registry = Registry::new();
        registry.add_action::<CreateWorktreeDefinition>().unwrap();

        let registration = registry.actions().next().unwrap();
        let schema = serde_json::to_value(registration.schema()).unwrap();

        assert_eq!(registration.id(), "agentboard/create-worktree");
        assert_eq!(schema["additionalProperties"], false);
        let required = schema["required"].as_array().unwrap();
        for field in ["repo", "root", "branch"] {
            assert!(required.iter().any(|value| value == field));
            assert!(schema["properties"][field].is_object());
            let mut inputs = BTreeMap::from([
                ("repo".into(), "/repo".into()),
                ("root".into(), "/root".into()),
                ("branch".into(), "feature".into()),
            ]);
            inputs.remove(field);
            assert!(registry
                .validate_action("agentboard/create-worktree", &inputs)
                .is_err());
        }
        assert!(registry
            .validate_action(
                "agentboard/create-worktree",
                &BTreeMap::from([
                    ("repo".into(), "/repo".into()),
                    ("root".into(), "/root".into()),
                    ("branch".into(), "feature".into()),
                    ("extra".into(), "value".into()),
                ]),
            )
            .is_err());

        let dir = tempdir().unwrap();
        let root = dir.path().join("not-created");
        registry
            .build_action(
                "agentboard/create-worktree",
                BTreeMap::from([
                    ("repo".into(), dir.path().display().to_string()),
                    ("root".into(), root.display().to_string()),
                    ("branch".into(), "feature".into()),
                ]),
            )
            .unwrap();
        assert!(!root.exists());
    }

    #[test]
    fn creates_attaches_and_reuses_worktrees() {
        let dir = tempdir().unwrap();
        let repo = dir.path().join("repo");
        let created = dir.path().join("created");
        let attached = dir.path().join("attached");
        init_repo(&repo);
        git(&repo, &["branch", "existing"]);
        let item = item();
        let context = ActionContext {
            workspace_id: "workspace",
            source_id: "issues",
            item: &item,
        };

        let run = CreateWorktreeDefinition::build(config(&repo, &created, "feature"))
            .unwrap()
            .execute(&context)
            .unwrap();
        assert!(run.success, "{:?} {:?}", run.stderr, run.message);
        assert_eq!(
            fs::read_to_string(created.join("README.md")).unwrap(),
            "test\n"
        );

        let run = CreateWorktreeDefinition::build(config(&repo, &created, "feature"))
            .unwrap()
            .execute(&context)
            .unwrap();
        assert!(run.success);
        assert_eq!(run.stdout, format!("reused {}\n", created.display()));

        let run = CreateWorktreeDefinition::build(config(&repo, &attached, "existing"))
            .unwrap()
            .execute(&context)
            .unwrap();
        assert!(run.success, "{:?} {:?}", run.stderr, run.message);
    }

    #[test]
    fn normalizes_expected_worktree_failures() {
        let dir = tempdir().unwrap();
        let repo = dir.path().join("repo");
        let root = dir.path().join("worktree");
        init_repo(&repo);
        let item = item();
        let context = ActionContext {
            workspace_id: "workspace",
            source_id: "issues",
            item: &item,
        };

        let first = CreateWorktreeDefinition::build(config(&repo, &root, "feature"))
            .unwrap()
            .execute(&context)
            .unwrap();
        assert!(first.success);

        let wrong_branch = CreateWorktreeDefinition::build(config(&repo, &root, "other"))
            .unwrap()
            .execute(&context)
            .unwrap();
        assert!(!wrong_branch.success);
        assert!(wrong_branch.stderr.contains("exists but is not worktree"));
        assert!(wrong_branch.message.is_some());

        let missing_repo = CreateWorktreeDefinition::build(config(
            &dir.path().join("missing-repo"),
            &dir.path().join("missing-worktree"),
            "feature",
        ))
        .unwrap()
        .execute(&context)
        .unwrap();
        assert!(!missing_repo.success);
        assert!(!missing_repo.stderr.is_empty());
        assert!(missing_repo.message.is_some());
    }

    #[test]
    fn checks_required_git_through_testable_boundary() {
        CreateWorktreeDefinition::health_check().unwrap();

        let error = check_command("git", &["--agentboard-invalid-option"]).unwrap_err();
        assert!(error.to_string().contains("required command git returned"));

        let error = check_command("agentboard-command-that-does-not-exist", &[]).unwrap_err();
        assert!(error
            .to_string()
            .contains("required command agentboard-command-that-does-not-exist not found"));
    }
}

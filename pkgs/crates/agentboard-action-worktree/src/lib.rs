use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, Output, Stdio},
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
pub struct WorktreeConfig {
    pub repo: String,
    pub root: String,
    pub branch: String,
}

pub struct WorktreeDefinition;

pub struct WorktreeAction {
    config: WorktreeConfig,
}

impl ActionDefinition for WorktreeDefinition {
    const ID: &'static str = "agentboard/worktree";
    type Config = WorktreeConfig;
    type Runtime = WorktreeAction;

    fn build(config: Self::Config) -> RuntimeResult<Self::Runtime> {
        Ok(WorktreeAction { config })
    }

    fn health_check() -> RuntimeResult<()> {
        check_command("git", &["--version"])
    }
}

impl Action for WorktreeAction {
    fn cached_success_is_valid(&self, _context: &ActionContext<'_>) -> bool {
        worktree_is_current(&self.config).unwrap_or(false)
    }

    fn execute(&self, _context: &ActionContext<'_>) -> RuntimeResult<ActionRun> {
        Ok(execute_worktree(&self.config))
    }
}

fn execute_worktree(config: &WorktreeConfig) -> ActionRun {
    match ensure_worktree(config) {
        Ok((stdout, stderr)) => successful_run(stdout, stderr),
        Err(error) => failed_run(error),
    }
}

fn worktree_is_current(config: &WorktreeConfig) -> Result<bool> {
    if !Path::new(&config.root).exists() {
        return Ok(false);
    }

    let repo_top_level = git_top_level(&config.repo)?;
    let root = fs::canonicalize(&config.root)?;
    if root != git_top_level(&config.root)? || root == repo_top_level {
        return Ok(false);
    }
    if git_common_dir(&config.repo)? != git_common_dir(&config.root)? {
        return Ok(false);
    }

    Ok(git_text(&config.root, &["branch", "--show-current"])? == config.branch)
}

fn ensure_worktree(config: &WorktreeConfig) -> Result<(String, String)> {
    let repo_top_level = git_top_level(&config.repo)?;
    validate_local_branch(&config.repo, &config.branch)?;

    if !Path::new(&config.root).exists() {
        return create_worktree(config);
    }

    let root = fs::canonicalize(&config.root)?;
    if root != git_top_level(&config.root)? {
        bail!("{} is not the worktree root", config.root);
    }
    if root == repo_top_level {
        bail!(
            "{} must be separate from repository {}",
            config.root,
            config.repo
        );
    }
    if git_common_dir(&config.repo)? != git_common_dir(&config.root)? {
        bail!(
            "{} belongs to a different repository than {}",
            config.root,
            config.repo
        );
    }

    if git_text(&config.root, &["branch", "--show-current"])? == config.branch {
        return Ok((format!("reused {}\n", config.root), String::new()));
    }
    if !git_text(&config.root, &["status", "--porcelain"])?.is_empty() {
        bail!("{} is dirty and cannot switch branches", config.root);
    }

    if local_branch_exists(&config.repo, &config.branch)? {
        run_git(&config.root, &["switch", &config.branch])
    } else {
        let head = git_text(&config.repo, &["rev-parse", "--verify", "HEAD"])?;
        run_git(&config.root, &["switch", "-c", &config.branch, &head])
    }
}

fn create_worktree(config: &WorktreeConfig) -> Result<(String, String)> {
    if local_branch_exists(&config.repo, &config.branch)? {
        run_git(
            &config.repo,
            &["worktree", "add", &config.root, &config.branch],
        )
    } else {
        let head = git_text(&config.repo, &["rev-parse", "--verify", "HEAD"])?;
        run_git(
            &config.repo,
            &["worktree", "add", "-b", &config.branch, &config.root, &head],
        )
    }
}

fn validate_local_branch(repo: &str, branch: &str) -> Result<()> {
    let output = ProcessCommand::new("git")
        .args(["-C", repo, "check-ref-format", "--branch", branch])
        .output()?;
    let normalized = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !output.status.success() || normalized != branch {
        bail!("{branch} is not a valid local branch name");
    }
    Ok(())
}

fn local_branch_exists(repo: &str, branch: &str) -> Result<bool> {
    let branch_ref = format!("refs/heads/{branch}");
    let status = ProcessCommand::new("git")
        .args(["-C", repo, "show-ref", "--verify", "--quiet", &branch_ref])
        .status()?;
    match status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => bail!("git show-ref failed with {status}"),
    }
}

fn git_top_level(path: &str) -> Result<PathBuf> {
    Ok(fs::canonicalize(git_text(
        path,
        &["rev-parse", "--show-toplevel"],
    )?)?)
}

fn git_common_dir(path: &str) -> Result<PathBuf> {
    Ok(fs::canonicalize(git_text(
        path,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?)?)
}

fn git_text(path: &str, args: &[&str]) -> Result<String> {
    let output = git_output(path, args)?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn run_git(path: &str, args: &[&str]) -> Result<(String, String)> {
    let output = git_output(path, args)?;
    Ok((cap(&output.stdout), cap(&output.stderr)))
}

fn git_output(path: &str, args: &[&str]) -> Result<Output> {
    let output = ProcessCommand::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()?;
    if !output.status.success() {
        let stderr = cap(&output.stderr);
        if stderr.is_empty() {
            bail!("git {} failed with {}", args.join(" "), output.status);
        }
        bail!("{stderr}");
    }
    Ok(output)
}

fn successful_run(stdout: String, stderr: String) -> ActionRun {
    ActionRun {
        success: true,
        stdout,
        stderr,
        message: None,
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

    fn git_stdout(repo: &Path, args: &[&str]) -> String {
        let output = ProcessCommand::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?} failed with {}",
            output.status
        );
        String::from_utf8(output.stdout).unwrap().trim().to_string()
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

    fn config(repo: &Path, root: &Path, branch: &str) -> WorktreeConfig {
        WorktreeConfig {
            repo: repo.display().to_string(),
            root: root.display().to_string(),
            branch: branch.into(),
        }
    }

    #[test]
    fn registers_worktree_config_schema() {
        let mut registry = Registry::new();
        registry.add_action::<WorktreeDefinition>().unwrap();

        let registration = registry.actions().next().unwrap();
        let schema = serde_json::to_value(registration.schema()).unwrap();

        assert_eq!(registration.id(), "agentboard/worktree");
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
                .validate_action("agentboard/worktree", &inputs)
                .is_err());
        }
        assert!(registry
            .validate_action(
                "agentboard/worktree",
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
                "agentboard/worktree",
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

        let run = WorktreeDefinition::build(config(&repo, &created, "feature"))
            .unwrap()
            .execute(&context)
            .unwrap();
        assert!(run.success, "{:?} {:?}", run.stderr, run.message);
        assert_eq!(
            fs::read_to_string(created.join("README.md")).unwrap(),
            "test\n"
        );

        fs::write(created.join("local.tmp"), "dirty\n").unwrap();
        let run = WorktreeDefinition::build(config(&repo, &created, "feature"))
            .unwrap()
            .execute(&context)
            .unwrap();
        assert!(run.success);
        assert_eq!(run.stdout, format!("reused {}\n", created.display()));

        let run = WorktreeDefinition::build(config(&repo, &attached, "existing"))
            .unwrap()
            .execute(&context)
            .unwrap();
        assert!(run.success, "{:?} {:?}", run.stderr, run.message);
    }

    #[test]
    fn cached_success_requires_current_worktree_state() {
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
        let action = WorktreeDefinition::build(config(&repo, &root, "feature")).unwrap();

        assert!(!action.cached_success_is_valid(&context));
        assert!(action.execute(&context).unwrap().success);
        assert!(action.cached_success_is_valid(&context));

        git(&repo, &["worktree", "remove", root.to_str().unwrap()]);
        assert!(!action.cached_success_is_valid(&context));
    }

    #[test]
    fn switches_clean_managed_worktree_to_existing_branch() {
        let dir = tempdir().unwrap();
        let repo = dir.path().join("repo");
        let root = dir.path().join("worktree");
        init_repo(&repo);
        git(&repo, &["branch", "existing"]);
        let item = item();
        let context = ActionContext {
            workspace_id: "workspace",
            source_id: "issues",
            item: &item,
        };

        let created = WorktreeDefinition::build(config(&repo, &root, "feature"))
            .unwrap()
            .execute(&context)
            .unwrap();
        assert!(
            created.success,
            "{:?} {:?}",
            created.stderr, created.message
        );

        let switched = WorktreeDefinition::build(config(&repo, &root, "existing"))
            .unwrap()
            .execute(&context)
            .unwrap();
        assert!(
            switched.success,
            "{:?} {:?}",
            switched.stderr, switched.message
        );
        assert_eq!(git_stdout(&root, &["branch", "--show-current"]), "existing");
    }

    #[test]
    fn creates_missing_branch_from_repo_head_in_existing_worktree() {
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

        let created = WorktreeDefinition::build(config(&repo, &root, "old"))
            .unwrap()
            .execute(&context)
            .unwrap();
        assert!(
            created.success,
            "{:?} {:?}",
            created.stderr, created.message
        );

        fs::write(repo.join("main.txt"), "main\n").unwrap();
        git(&repo, &["add", "main.txt"]);
        git(&repo, &["commit", "-m", "advance main"]);
        let expected_head = git_stdout(&repo, &["rev-parse", "HEAD"]);

        let switched = WorktreeDefinition::build(config(&repo, &root, "new"))
            .unwrap()
            .execute(&context)
            .unwrap();
        assert!(
            switched.success,
            "{:?} {:?}",
            switched.stderr, switched.message
        );
        assert_eq!(git_stdout(&root, &["branch", "--show-current"]), "new");
        assert_eq!(git_stdout(&root, &["rev-parse", "HEAD"]), expected_head);
    }

    #[test]
    fn rejects_subdirectory_as_managed_worktree_root() {
        let dir = tempdir().unwrap();
        let repo = dir.path().join("repo");
        let root = dir.path().join("worktree");
        init_repo(&repo);
        git(&repo, &["branch", "other"]);
        let item = item();
        let context = ActionContext {
            workspace_id: "workspace",
            source_id: "issues",
            item: &item,
        };

        let created = WorktreeDefinition::build(config(&repo, &root, "feature"))
            .unwrap()
            .execute(&context)
            .unwrap();
        assert!(
            created.success,
            "{:?} {:?}",
            created.stderr, created.message
        );
        let nested = root.join("nested");
        fs::create_dir(&nested).unwrap();

        let switched = WorktreeDefinition::build(config(&repo, &nested, "other"))
            .unwrap()
            .execute(&context)
            .unwrap();
        assert!(!switched.success);
        assert_eq!(git_stdout(&root, &["branch", "--show-current"]), "feature");
    }

    #[test]
    fn rejects_managed_worktree_from_different_repository() {
        let dir = tempdir().unwrap();
        let repo = dir.path().join("repo");
        let other_repo = dir.path().join("other-repo");
        let root = dir.path().join("worktree");
        init_repo(&repo);
        init_repo(&other_repo);
        let item = item();
        let context = ActionContext {
            workspace_id: "workspace",
            source_id: "issues",
            item: &item,
        };

        let created = WorktreeDefinition::build(config(&other_repo, &root, "feature"))
            .unwrap()
            .execute(&context)
            .unwrap();
        assert!(
            created.success,
            "{:?} {:?}",
            created.stderr, created.message
        );

        let reused = WorktreeDefinition::build(config(&repo, &root, "feature"))
            .unwrap()
            .execute(&context)
            .unwrap();
        assert!(!reused.success);
        assert!(reused.stderr.contains("different repository"));
    }

    #[test]
    fn rejects_configured_repository_as_managed_root() {
        let dir = tempdir().unwrap();
        let repo = dir.path().join("repo");
        init_repo(&repo);
        let branch = git_stdout(&repo, &["branch", "--show-current"]);
        let item = item();
        let context = ActionContext {
            workspace_id: "workspace",
            source_id: "issues",
            item: &item,
        };

        let run = WorktreeDefinition::build(config(&repo, &repo, &branch))
            .unwrap()
            .execute(&context)
            .unwrap();
        assert!(!run.success);
        assert!(run.stderr.contains("separate from repository"));
    }

    #[test]
    fn refuses_to_switch_dirty_managed_worktree() {
        let dir = tempdir().unwrap();
        let repo = dir.path().join("repo");
        let root = dir.path().join("worktree");
        init_repo(&repo);
        git(&repo, &["branch", "target"]);
        let item = item();
        let context = ActionContext {
            workspace_id: "workspace",
            source_id: "issues",
            item: &item,
        };

        let created = WorktreeDefinition::build(config(&repo, &root, "feature"))
            .unwrap()
            .execute(&context)
            .unwrap();
        assert!(
            created.success,
            "{:?} {:?}",
            created.stderr, created.message
        );

        fs::write(root.join("untracked.txt"), "local\n").unwrap();
        let untracked = WorktreeDefinition::build(config(&repo, &root, "target"))
            .unwrap()
            .execute(&context)
            .unwrap();
        assert!(!untracked.success);
        assert!(untracked.stderr.contains("dirty"));
        assert_eq!(git_stdout(&root, &["branch", "--show-current"]), "feature");

        fs::remove_file(root.join("untracked.txt")).unwrap();
        fs::write(root.join("README.md"), "changed\n").unwrap();
        let tracked = WorktreeDefinition::build(config(&repo, &root, "target"))
            .unwrap()
            .execute(&context)
            .unwrap();
        assert!(!tracked.success);
        assert!(tracked.stderr.contains("dirty"));
        assert_eq!(git_stdout(&root, &["branch", "--show-current"]), "feature");
    }

    #[test]
    fn allows_ignored_files_when_switching_managed_worktree() {
        let dir = tempdir().unwrap();
        let repo = dir.path().join("repo");
        let root = dir.path().join("worktree");
        init_repo(&repo);
        fs::write(repo.join(".gitignore"), "ignored.txt\n").unwrap();
        git(&repo, &["add", ".gitignore"]);
        git(&repo, &["commit", "-m", "ignore local file"]);
        git(&repo, &["branch", "target"]);
        let item = item();
        let context = ActionContext {
            workspace_id: "workspace",
            source_id: "issues",
            item: &item,
        };

        let created = WorktreeDefinition::build(config(&repo, &root, "feature"))
            .unwrap()
            .execute(&context)
            .unwrap();
        assert!(
            created.success,
            "{:?} {:?}",
            created.stderr, created.message
        );
        fs::write(root.join("ignored.txt"), "local\n").unwrap();

        let switched = WorktreeDefinition::build(config(&repo, &root, "target"))
            .unwrap()
            .execute(&context)
            .unwrap();
        assert!(
            switched.success,
            "{:?} {:?}",
            switched.stderr, switched.message
        );
        assert_eq!(git_stdout(&root, &["branch", "--show-current"]), "target");
    }

    #[test]
    fn creates_local_branch_when_only_tag_matches_name() {
        let dir = tempdir().unwrap();
        let repo = dir.path().join("repo");
        let root = dir.path().join("worktree");
        init_repo(&repo);
        git(&repo, &["tag", "release"]);
        let item = item();
        let context = ActionContext {
            workspace_id: "workspace",
            source_id: "issues",
            item: &item,
        };

        let run = WorktreeDefinition::build(config(&repo, &root, "release"))
            .unwrap()
            .execute(&context)
            .unwrap();
        assert!(run.success, "{:?} {:?}", run.stderr, run.message);
        assert_eq!(git_stdout(&root, &["branch", "--show-current"]), "release");
        git(&repo, &["show-ref", "--verify", "refs/heads/release"]);
    }

    #[test]
    fn refuses_branch_checked_out_in_another_worktree() {
        let dir = tempdir().unwrap();
        let repo = dir.path().join("repo");
        let root = dir.path().join("worktree");
        let other = dir.path().join("other-worktree");
        init_repo(&repo);
        git(&repo, &["branch", "target"]);
        git(
            &repo,
            &["worktree", "add", other.to_str().unwrap(), "target"],
        );
        let item = item();
        let context = ActionContext {
            workspace_id: "workspace",
            source_id: "issues",
            item: &item,
        };

        let created = WorktreeDefinition::build(config(&repo, &root, "feature"))
            .unwrap()
            .execute(&context)
            .unwrap();
        assert!(
            created.success,
            "{:?} {:?}",
            created.stderr, created.message
        );

        let switched = WorktreeDefinition::build(config(&repo, &root, "target"))
            .unwrap()
            .execute(&context)
            .unwrap();
        assert!(!switched.success);
        assert_eq!(git_stdout(&root, &["branch", "--show-current"]), "feature");
    }

    #[test]
    fn rejects_invalid_local_branch_name() {
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

        let run = WorktreeDefinition::build(config(&repo, &root, "bad..branch"))
            .unwrap()
            .execute(&context)
            .unwrap();
        assert!(!run.success);
        assert!(run.stderr.contains("valid local branch"));
        assert!(!root.exists());
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

        fs::create_dir(&root).unwrap();
        let invalid_root = WorktreeDefinition::build(config(&repo, &root, "feature"))
            .unwrap()
            .execute(&context)
            .unwrap();
        assert!(!invalid_root.success);
        assert!(!invalid_root.stderr.is_empty());
        assert!(invalid_root.message.is_some());

        let missing_repo = WorktreeDefinition::build(config(
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
        WorktreeDefinition::health_check().unwrap();

        let error = check_command("git", &["--agentboard-invalid-option"]).unwrap_err();
        assert!(error.to_string().contains("required command git returned"));

        let error = check_command("agentboard-command-that-does-not-exist", &[]).unwrap_err();
        assert!(error
            .to_string()
            .contains("required command agentboard-command-that-does-not-exist not found"));
    }
}

use std::{collections::BTreeMap, path::Path, process::Command as ProcessCommand};

use anyhow::{bail, Result};

use agentboard_core::cap;

pub fn create_worktree(inputs: &BTreeMap<String, String>) -> Result<(String, String)> {
    let repo = inputs.get("repo").unwrap();
    let root = inputs.get("root").unwrap();
    let branch = inputs.get("branch").unwrap();
    if Path::new(root).exists() {
        let out = ProcessCommand::new("git")
            .args(["-C", root, "branch", "--show-current"])
            .output()?;
        let current = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if out.status.success() && current == *branch {
            return Ok((format!("reused {root}\n"), String::new()));
        }
        bail!("{} exists but is not worktree for branch {}", root, branch);
    }
    let exists = ProcessCommand::new("git")
        .args(["-C", repo, "rev-parse", "--verify", branch])
        .output()?
        .status
        .success();
    let mut cmd = ProcessCommand::new("git");
    cmd.arg("-C").arg(repo).arg("worktree").arg("add");
    if exists {
        cmd.arg(root).arg(branch);
    } else {
        cmd.arg("-b").arg(branch).arg(root);
    }
    let out = cmd.output()?;
    if !out.status.success() {
        bail!(
            "git worktree failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok((cap(&out.stdout), cap(&out.stderr)))
}

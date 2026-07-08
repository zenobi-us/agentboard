use std::{
    collections::BTreeMap,
    process::{Command as ProcessCommand, Stdio},
};

use anyhow::Result;

use agentboard_core::{
    cap,
    model::{Item, SourceConfig, Workspace},
    ActionRun,
};

pub fn run_cmd(
    ws: &Workspace,
    source: &SourceConfig,
    item: &Item,
    inputs: &BTreeMap<String, String>,
) -> Result<ActionRun> {
    let cmd = inputs.get("cmd").unwrap();
    let mut c = ProcessCommand::new("sh");
    c.arg("-c")
        .arg(cmd)
        .stdin(Stdio::null())
        .env("AGENTBOARD_WORKSPACE_ID", &ws.id)
        .env("AGENTBOARD_SOURCE_ID", &source.id)
        .env("AGENTBOARD_ITEM_ID", &item.id);
    if let Some(cwd) = inputs.get("cwd") {
        c.current_dir(cwd);
    }
    let out = c.output()?;
    let success = out.status.success();
    Ok(ActionRun {
        success,
        stdout: cap(&out.stdout),
        stderr: cap(&out.stderr),
        message: (!success).then(|| format!("command exited with {}", out.status)),
    })
}

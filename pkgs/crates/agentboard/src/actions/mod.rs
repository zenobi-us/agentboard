pub mod run_cmd;
pub mod worktree;

use anyhow::Result;
use chrono::Utc;

use crate::{
    model::{ActionAttempt, ActionConfig, Item, SourceConfig, Workspace},
    template::RenderedAction,
};
use run_cmd::run_cmd;
use worktree::create_worktree;

pub const STDOUT_LIMIT: usize = 64 * 1024;

pub struct ActionRun {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub message: Option<String>,
}

pub fn execute_action(
    ws: &Workspace,
    source: &SourceConfig,
    item: &Item,
    idx: usize,
    action: &ActionConfig,
    rendered: RenderedAction,
) -> Result<ActionAttempt> {
    let run = match action.uses.as_str() {
        "agentboard/run-cmd" => run_cmd(ws, source, item, &rendered.inputs)?,
        "agentboard/create-worktree" => match create_worktree(&rendered.inputs) {
            Ok((stdout, stderr)) => ActionRun {
                success: true,
                stdout,
                stderr,
                message: None,
            },
            Err(err) => ActionRun {
                success: false,
                stdout: String::new(),
                stderr: format!("{err:#}"),
                message: Some(err.to_string()),
            },
        },
        _ => unreachable!(),
    };
    Ok(ActionAttempt {
        ts: Utc::now().to_rfc3339(),
        source_id: source.id.clone(),
        item_id: item.id.clone(),
        source_action_index: idx,
        uses: action.uses.clone(),
        rendered_action_hash: rendered.hash,
        success: run.success,
        stdout: run.stdout,
        stderr: run.stderr,
        message: run.message,
    })
}

pub fn cap(bytes: &[u8]) -> String {
    String::from_utf8_lossy(&bytes[..bytes.len().min(STDOUT_LIMIT)]).to_string()
}

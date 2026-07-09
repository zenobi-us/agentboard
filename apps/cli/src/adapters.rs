use agentboard_core::{
    model::{ActionAttempt, ActionConfig, Item, SourceConfig, SourceKind, Workspace},
    RenderedAction,
};
use anyhow::Result;
use chrono::Utc;

/// Dispatch collection to the crate that owns the configured source kind.
pub async fn collect_items(source: &SourceConfig) -> Result<Vec<Item>> {
    match &source.source {
        SourceKind::Qmd { .. } => agentboard_source_qmd::collect_items(source).await,
        SourceKind::Jira { .. } => agentboard_source_jira::collect_items(source).await,
        SourceKind::Github { .. } => agentboard_source_github::collect_items(source).await,
    }
}

/// Dispatch one rendered action to its built-in action crate and normalize the result.
pub fn execute_action(
    ws: &Workspace,
    source: &SourceConfig,
    item: &Item,
    idx: usize,
    action: &ActionConfig,
    rendered: RenderedAction,
) -> Result<ActionAttempt> {
    let run = match action.uses.as_str() {
        "agentboard/run-cmd" => {
            agentboard_action_run_cmd::run_cmd(ws, source, item, &rendered.inputs)?
        }
        "agentboard/create-worktree" => {
            match agentboard_action_worktree::create_worktree(&rendered.inputs) {
                Ok((stdout, stderr)) => agentboard_core::ActionRun {
                    success: true,
                    stdout,
                    stderr,
                    message: None,
                },
                Err(err) => agentboard_core::ActionRun {
                    success: false,
                    stdout: String::new(),
                    stderr: format!("{err:#}"),
                    message: Some(err.to_string()),
                },
            }
        }
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

use agentboard_core::{
    model::{ActionAttempt, ActionConfig, Item, SourceConfig, SourceKind, Workspace},
    RenderedAction,
};
use anyhow::Result;
use chrono::Utc;

pub struct SourceInspection {
    pub items: Vec<Item>,
    pub available: Option<usize>,
    pub limit: usize,
}

/// Dispatch collection to the crate that owns the configured source kind.
pub async fn collect_items(source: &SourceConfig) -> Result<Vec<Item>> {
    match &source.source {
        SourceKind::Qmd { .. } => agentboard_source_qmd::collect_items(source).await,
        SourceKind::Jira { .. } => agentboard_source_jira::collect_items(source).await,
        SourceKind::Github { .. } => agentboard_source_github::collect_items(source).await,
    }
}

/// Collect configured Items and expose an upstream match count when supported.
pub async fn inspect_source(source: &SourceConfig) -> Result<SourceInspection> {
    let limit = match &source.source {
        SourceKind::Qmd { limit, .. }
        | SourceKind::Jira { limit, .. }
        | SourceKind::Github { limit, .. } => *limit,
    };
    let (items, available) = match &source.source {
        SourceKind::Github { .. } => {
            let (items, available) = agentboard_source_github::inspect_items(source).await?;
            (items, Some(available))
        }
        SourceKind::Qmd { .. } => (agentboard_source_qmd::collect_items(source).await?, None),
        SourceKind::Jira { .. } => (agentboard_source_jira::collect_items(source).await?, None),
    };
    Ok(SourceInspection {
        items,
        available,
        limit,
    })
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

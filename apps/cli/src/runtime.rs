use std::{sync::Arc, time::Duration};

use agentboard_core::model::{SourceConfig, Workspace};
use anyhow::{bail, Result};

use crate::{
    adapters::{collect_items, execute_action},
    store::{acquire_lock, action_key, append_action, append_items, successful_actions},
    template::render_action,
};

/// Execute one workspace run.
///
/// A normal run holds the workspace lock. Dry runs skip locking and store writes.
pub async fn run_once(ws: &Workspace, dry_run: bool) -> Result<()> {
    let _lock = if dry_run {
        None
    } else {
        Some(acquire_lock(ws)?)
    };
    run_sources(ws, dry_run).await
}

/// Repeatedly execute one workspace run until Ctrl-C.
pub async fn watch(ws: Workspace, delay: Duration) -> Result<()> {
    let _lock = acquire_lock(&ws)?;
    loop {
        if let Err(err) = run_sources(&ws, false).await {
            eprintln!("run failed: {err:#}");
        }
        tokio::select! {
            _ = tokio::time::sleep(delay) => {}
            _ = tokio::signal::ctrl_c() => return Ok(()),
        }
    }
}

// Run source pipelines concurrently, but report a failed process if any source fails.
async fn run_sources(ws: &Workspace, dry_run: bool) -> Result<()> {
    let ws = Arc::new(ws.clone());
    let mut handles = Vec::new();
    for source in ws.config.sources.clone() {
        let ws = Arc::clone(&ws);
        handles.push(tokio::spawn(async move {
            run_source(&ws, &source, dry_run)
                .await
                .map_err(|err| (source.id, err))
        }));
    }

    let mut failed = false;
    for handle in handles {
        match handle.await? {
            Ok(ok) => failed |= !ok,
            Err((source_id, err)) => {
                eprintln!("source {source_id} failed: {err:#}");
                failed = true;
            }
        }
    }
    if failed {
        bail!("run completed with failures");
    }
    Ok(())
}

// Run one source pipeline: collect, append observations, then execute pending actions serially.
async fn run_source(ws: &Workspace, source: &SourceConfig, dry_run: bool) -> Result<bool> {
    let mut items = collect_items(source).await?;
    items.sort_by(|a, b| a.id.cmp(&b.id));
    if !dry_run {
        append_items(ws, source, &items)?;
    }
    let successes = successful_actions(ws, source)?;
    let mut ok = true;
    for item in items {
        for (idx, action) in source.actions.iter().enumerate() {
            let rendered = render_action(ws, source, &item, idx, action)?;
            let key = action_key(&source.id, &item.id, idx, &rendered.hash);
            if successes.contains(&key) {
                continue;
            }
            if dry_run {
                println!(
                    "{} {} action#{idx} {} {}",
                    source.id,
                    item.id,
                    action.uses,
                    serde_json::to_string(&rendered.inputs)?
                );
                continue;
            }
            let attempt = execute_action(ws, source, &item, idx, action, rendered)?;
            let success = attempt.success;
            append_action(ws, source, &attempt)?;
            if !success {
                ok = false;
                break;
            }
        }
    }
    Ok(ok)
}

/// Parse an interval string accepted by `watch`.
///
/// Currently accepts seconds with or without a trailing `s`.
pub fn parse_duration(s: &str) -> Result<Duration> {
    let secs = s.strip_suffix('s').unwrap_or(s).parse()?;
    Ok(Duration::from_secs(secs))
}

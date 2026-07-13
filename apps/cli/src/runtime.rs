use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use agentboard_core::model::{SourceConfig, Workspace};
use anyhow::{bail, Result};
use serde_json::json;

use crate::{
    adapters::{collect_items, execute_action},
    output::Output,
    store::{acquire_lock, action_key, append_action, append_items, successful_actions},
    template::render_action,
};

#[derive(Default)]
struct RunSummary {
    items: usize,
    attempted: usize,
    skipped: usize,
    succeeded: usize,
    failed: usize,
}

/// Execute one Workspace Run.
///
/// A normal Run holds the Workspace lock. Dry runs skip locking and Store writes.
pub async fn run_once(ws: &Workspace, dry_run: bool, output: &Output) -> Result<()> {
    let _lock = if dry_run {
        None
    } else {
        Some(acquire_lock(ws)?)
    };
    run_sources(ws, dry_run, output).await
}

/// Repeatedly execute one Workspace Run until Ctrl-C.
pub async fn watch(ws: Workspace, delay: Duration, output: &Output) -> Result<()> {
    let _lock = acquire_lock(&ws)?;
    let mut cycle = 1_u64;
    loop {
        output.info(
            "watch.cycle.start",
            &format!("watch {} cycle {cycle} starting", ws.id),
            json!({"workspace": ws.id, "cycle": cycle}),
        )?;
        match run_sources(&ws, false, output).await {
            Ok(()) => output.success(
                "watch.cycle.complete",
                &format!("watch {} cycle {cycle} complete", ws.id),
                json!({"workspace": ws.id, "cycle": cycle, "outcome": "pass"}),
            )?,
            Err(err) => output.error(
                "watch.cycle.failed",
                &format!("watch {} cycle {cycle} failed: {err:#}", ws.id),
                json!({"workspace": ws.id, "cycle": cycle, "outcome": "fail", "error": format!("{err:#}")}),
            )?,
        }
        output.info(
            "watch.wait",
            &format!("watch {} next Run in {}s", ws.id, delay.as_secs()),
            json!({"workspace": ws.id, "cycle": cycle, "delay_seconds": delay.as_secs()}),
        )?;
        tokio::select! {
            _ = tokio::time::sleep(delay) => cycle += 1,
            _ = tokio::signal::ctrl_c() => {
                output.success(
                    "watch.stop",
                    &format!("watch {} stopped", ws.id),
                    json!({"workspace": ws.id, "cycle": cycle}),
                )?;
                return Ok(());
            },
        }
    }
}

// Run Source pipelines concurrently, but report a failed process if any Source fails.
async fn run_sources(ws: &Workspace, dry_run: bool, output: &Output) -> Result<()> {
    let run_id = output.next_run_id();
    let started = Instant::now();
    output.info(
        "run.start",
        &format!("run {} starting", ws.id),
        json!({"workspace": ws.id, "run": run_id, "dry_run": dry_run}),
    )?;
    let ws = Arc::new(ws.clone());
    let mut handles = Vec::new();
    for source in ws.config.sources.clone() {
        let ws = Arc::clone(&ws);
        let output = output.clone();
        let run_id = run_id.clone();
        handles.push(tokio::spawn(async move {
            run_source(&ws, &source, dry_run, &run_id, &output)
                .await
                .map_err(|err| (source.id, err))
        }));
    }

    let mut summary = RunSummary::default();
    for handle in handles {
        match handle.await? {
            Ok(source) => {
                summary.items += source.items;
                summary.attempted += source.attempted;
                summary.skipped += source.skipped;
                summary.succeeded += source.succeeded;
                summary.failed += source.failed;
            }
            Err((source_id, err)) => {
                output.error(
                    "source.failed",
                    &format!("source {source_id} failed: {err:#}"),
                    json!({"workspace": ws.id, "run": run_id, "source": source_id, "error": format!("{err:#}")}),
                )?;
                summary.failed += 1;
            }
        }
    }
    let duration_ms = started.elapsed().as_millis();
    let metadata = json!({
        "workspace": ws.id,
        "run": run_id,
        "items": summary.items,
        "attempted": summary.attempted,
        "skipped": summary.skipped,
        "succeeded": summary.succeeded,
        "failed": summary.failed,
        "duration_ms": duration_ms,
    });
    let message = format!(
        "run {} complete: {} items, {} attempted, {} skipped, {} succeeded, {} failed, {}ms",
        ws.id,
        summary.items,
        summary.attempted,
        summary.skipped,
        summary.succeeded,
        summary.failed,
        duration_ms
    );
    if summary.failed > 0 {
        output.error("run.failed", &message, metadata)?;
        bail!("run completed with failures");
    }
    output.success("run.complete", &message, metadata)?;
    Ok(())
}

// Run one Source pipeline: collect, append observations, then execute pending Actions serially.
async fn run_source(
    ws: &Workspace,
    source: &SourceConfig,
    dry_run: bool,
    run_id: &str,
    output: &Output,
) -> Result<RunSummary> {
    let started = Instant::now();
    output.detail(
        "source.start",
        &format!("source {} collecting", source.id),
        json!({"workspace": ws.id, "run": run_id, "source": source.id}),
    )?;
    let mut items = collect_items(source).await?;
    items.sort_by(|a, b| a.id.cmp(&b.id));
    if !dry_run {
        append_items(ws, source, &items)?;
    }
    let successes = successful_actions(ws, source)?;
    let mut summary = RunSummary {
        items: items.len(),
        ..Default::default()
    };
    for item in items {
        for (idx, action) in source.actions.iter().enumerate() {
            let rendered = render_action(ws, source, &item, idx, action)?;
            let key = action_key(&source.id, &item.id, idx, &rendered.hash);
            if successes.contains(&key) {
                summary.skipped += 1;
                output.detail(
                    "action.skipped",
                    &format!("{} {} action#{idx} skipped", source.id, item.id),
                    json!({"workspace": ws.id, "run": run_id, "source": source.id, "item": item.id, "action_index": idx, "uses": action.uses}),
                )?;
                continue;
            }
            summary.attempted += 1;
            if dry_run {
                summary.succeeded += 1;
                println!(
                    "{} {} action#{idx} {} {}",
                    source.id,
                    item.id,
                    action.uses,
                    serde_json::to_string(&rendered.inputs)?
                );
                output.detail(
                    "action.dry-run",
                    &format!("{} {} action#{idx} {} would run", source.id, item.id, action.uses),
                    json!({"workspace": ws.id, "run": run_id, "source": source.id, "item": item.id, "action_index": idx, "uses": action.uses}),
                )?;
                continue;
            }
            let attempt = execute_action(ws, source, &item, idx, action, rendered)?;
            append_action(ws, source, &attempt)?;
            if attempt.success {
                summary.succeeded += 1;
                output.detail(
                    "action.succeeded",
                    &format!("{} {} action#{idx} {} succeeded", source.id, item.id, action.uses),
                    json!({"workspace": ws.id, "run": run_id, "source": source.id, "item": item.id, "action_index": idx, "uses": action.uses}),
                )?;
                if !attempt.stdout.is_empty() {
                    output.detail("action.stdout", &attempt.stdout, json!({"workspace": ws.id, "run": run_id, "source": source.id, "item": item.id, "action_index": idx}))?;
                }
                if !attempt.stderr.is_empty() {
                    output.detail("action.stderr", &attempt.stderr, json!({"workspace": ws.id, "run": run_id, "source": source.id, "item": item.id, "action_index": idx}))?;
                }
            } else {
                summary.failed += 1;
                output.error(
                    "action.failed",
                    &format!("{} {} action#{idx} {} failed", source.id, item.id, action.uses),
                    json!({"workspace": ws.id, "run": run_id, "source": source.id, "item": item.id, "action_index": idx, "uses": action.uses}),
                )?;
                if !attempt.stdout.is_empty() {
                    output.error("action.stdout", &attempt.stdout, json!({"workspace": ws.id, "run": run_id, "source": source.id, "item": item.id, "action_index": idx}))?;
                }
                if !attempt.stderr.is_empty() {
                    output.error("action.stderr", &attempt.stderr, json!({"workspace": ws.id, "run": run_id, "source": source.id, "item": item.id, "action_index": idx}))?;
                }
                break;
            }
        }
    }
    output.info(
        "source.complete",
        &format!(
            "source {} complete: {} items, {} attempted, {} skipped, {} failed",
            source.id, summary.items, summary.attempted, summary.skipped, summary.failed
        ),
        json!({
            "workspace": ws.id,
            "run": run_id,
            "source": source.id,
            "items": summary.items,
            "attempted": summary.attempted,
            "skipped": summary.skipped,
            "succeeded": summary.succeeded,
            "failed": summary.failed,
            "duration_ms": started.elapsed().as_millis(),
        }),
    )?;
    Ok(summary)
}

/// Parse an interval string accepted by `watch`.
///
/// Currently accepts seconds with or without a trailing `s`.
pub fn parse_duration(s: &str) -> Result<Duration> {
    let secs = s.strip_suffix('s').unwrap_or(s).parse()?;
    Ok(Duration::from_secs(secs))
}

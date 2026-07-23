use std::{
    future::Future,
    sync::Arc,
    time::{Duration, Instant},
};

use agentboard_core::{
    model::{ActionAttempt, ActionConfig, Item, Workspace, WorkspaceSource},
    registry::{ActionContext, Registry, SourceContext},
    RenderedAction,
};
use anyhow::{bail, Result};
use chrono::Utc;
use serde_json::json;

use crate::{
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

#[derive(Debug, Eq, PartialEq)]
enum WaitOutcome {
    Elapsed,
    Interrupted,
}

/// Execute one Workspace Run.
///
/// A normal Run holds the Workspace lock. Dry runs skip locking and Store writes.
pub async fn run_once(
    ws: &Workspace,
    registry: Arc<Registry>,
    dry_run: bool,
    output: &Output,
) -> Result<()> {
    let _lock = if dry_run {
        None
    } else {
        Some(acquire_lock(ws)?)
    };
    run_sources(ws, registry, dry_run, output).await
}

/// Repeatedly execute one Workspace Run until Ctrl-C.
pub async fn watch(
    ws: Workspace,
    registry: Arc<Registry>,
    delay: Duration,
    output: &Output,
) -> Result<()> {
    let _lock = acquire_lock(&ws)?;
    let mut cycle = 1_u64;
    loop {
        output.info(
            "watch.cycle.start",
            &format!("watch {} cycle {cycle} starting", ws.id),
            json!({"workspace": ws.id, "cycle": cycle}),
        )?;
        match run_sources(&ws, Arc::clone(&registry), false, output).await {
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
        match wait_for_next_cycle(&ws.id, cycle, delay, output, async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?
        {
            WaitOutcome::Elapsed => cycle += 1,
            WaitOutcome::Interrupted => {
                output.success(
                    "watch.stop",
                    &format!("watch {} stopped", ws.id),
                    json!({"workspace": ws.id, "cycle": cycle}),
                )?;
                return Ok(());
            }
        }
    }
}

async fn wait_for_next_cycle<F>(
    workspace: &str,
    cycle: u64,
    delay: Duration,
    output: &Output,
    stop: F,
) -> Result<WaitOutcome>
where
    F: Future<Output = ()>,
{
    let started = tokio::time::Instant::now();
    let deadline = started + delay;
    output.transient_info(
        "watch.wait",
        &format!("watch {workspace} next Run in {}s", delay.as_secs()),
        json!({"workspace": workspace, "cycle": cycle, "delay_seconds": delay.as_secs()}),
    )?;

    tokio::pin!(stop);
    let outcome = if output.transient_enabled() {
        let mut next_tick = started + Duration::from_secs(1);
        loop {
            let wake = next_tick.min(deadline);
            tokio::select! {
                _ = tokio::time::sleep_until(wake) => {
                    let now = tokio::time::Instant::now();
                    if now >= deadline {
                        break WaitOutcome::Elapsed;
                    }
                    let remaining = deadline.saturating_duration_since(now);
                    let seconds = remaining.as_secs() + u64::from(remaining.subsec_nanos() > 0);
                    output.update_transient(&format!("watch {workspace} next Run in {seconds}s"))?;
                    next_tick += Duration::from_secs(1);
                    while next_tick <= now {
                        next_tick += Duration::from_secs(1);
                    }
                }
                _ = &mut stop => break WaitOutcome::Interrupted,
            }
        }
    } else {
        tokio::select! {
            _ = tokio::time::sleep_until(deadline) => WaitOutcome::Elapsed,
            _ = &mut stop => WaitOutcome::Interrupted,
        }
    };
    output.finish_transient()?;
    Ok(outcome)
}

// Run Source pipelines concurrently, but report a failed process if any Source fails.
async fn run_sources(
    ws: &Workspace,
    registry: Arc<Registry>,
    dry_run: bool,
    output: &Output,
) -> Result<()> {
    let run_id = output.next_run_id();
    let started = Instant::now();
    output.info(
        "run.start",
        &format!("run {} starting", ws.id),
        json!({"workspace": ws.id, "run": run_id, "dry_run": dry_run}),
    )?;
    let ws = Arc::new(ws.clone());
    let mut handles = Vec::new();
    for source in ws.sources.clone() {
        let ws = Arc::clone(&ws);
        let registry = Arc::clone(&registry);
        let output = output.clone();
        let run_id = run_id.clone();
        handles.push(tokio::spawn(async move {
            run_source(&ws, &source, &registry, dry_run, &run_id, &output)
                .await
                .map_err(|err| (source.configured.id, err))
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
    source: &WorkspaceSource,
    registry: &Registry,
    dry_run: bool,
    run_id: &str,
    output: &Output,
) -> Result<RunSummary> {
    let source_id = &source.configured.id;
    let started = Instant::now();
    output.detail(
        "source.start",
        &format!("source {source_id} collecting"),
        json!({"workspace": ws.id, "run": run_id, "source": source_id}),
    )?;
    let context = SourceContext { source_id };
    let mut items = source.built.runtime().collect(&context).await?.items;
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
        for (idx, action) in source.configured.actions.iter().enumerate() {
            let rendered = render_action(ws, source, &item, idx, action)?;
            let key = action_key(source_id, &item.id, idx, &rendered.hash);
            if successes.contains(&key) {
                summary.skipped += 1;
                output.detail(
                    "action.skipped",
                    &format!("{source_id} {} action#{idx} skipped", item.id),
                    json!({"workspace": ws.id, "run": run_id, "source": source_id, "item": item.id, "action_index": idx, "uses": action.uses}),
                )?;
                continue;
            }
            summary.attempted += 1;
            if dry_run {
                summary.succeeded += 1;
                println!(
                    "{} {} action#{idx} {} {}",
                    source_id,
                    item.id,
                    action.uses,
                    serde_json::to_string(&rendered.inputs)?
                );
                output.detail(
                    "action.dry-run",
                    &format!("{source_id} {} action#{idx} {} would run", item.id, action.uses),
                    json!({"workspace": ws.id, "run": run_id, "source": source_id, "item": item.id, "action_index": idx, "uses": action.uses}),
                )?;
                continue;
            }
            let attempt =
                execute_action_attempt(registry, &ws.id, source_id, &item, idx, action, rendered);
            append_action(ws, source, &attempt)?;
            if attempt.success {
                summary.succeeded += 1;
                output.detail(
                    "action.succeeded",
                    &format!("{source_id} {} action#{idx} {} succeeded", item.id, action.uses),
                    json!({"workspace": ws.id, "run": run_id, "source": source_id, "item": item.id, "action_index": idx, "uses": action.uses}),
                )?;
                if !attempt.stdout.is_empty() {
                    output.detail("action.stdout", &attempt.stdout, json!({"workspace": ws.id, "run": run_id, "source": source_id, "item": item.id, "action_index": idx}))?;
                }
                if !attempt.stderr.is_empty() {
                    output.detail("action.stderr", &attempt.stderr, json!({"workspace": ws.id, "run": run_id, "source": source_id, "item": item.id, "action_index": idx}))?;
                }
            } else {
                summary.failed += 1;
                output.error(
                    "action.failed",
                    &format!("{source_id} {} action#{idx} {} failed", item.id, action.uses),
                    json!({"workspace": ws.id, "run": run_id, "source": source_id, "item": item.id, "action_index": idx, "uses": action.uses}),
                )?;
                if !attempt.stdout.is_empty() {
                    output.error("action.stdout", &attempt.stdout, json!({"workspace": ws.id, "run": run_id, "source": source_id, "item": item.id, "action_index": idx}))?;
                }
                if !attempt.stderr.is_empty() {
                    output.error("action.stderr", &attempt.stderr, json!({"workspace": ws.id, "run": run_id, "source": source_id, "item": item.id, "action_index": idx}))?;
                }
                break;
            }
        }
    }
    output.info(
        "source.complete",
        &format!(
            "source {source_id} complete: {} items, {} attempted, {} skipped, {} failed",
            summary.items, summary.attempted, summary.skipped, summary.failed
        ),
        json!({
            "workspace": ws.id,
            "run": run_id,
            "source": source_id,
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

fn execute_action_attempt(
    registry: &Registry,
    workspace_id: &str,
    source_id: &str,
    item: &Item,
    idx: usize,
    action: &ActionConfig,
    rendered: RenderedAction,
) -> ActionAttempt {
    let hash = rendered.hash;
    let run = registry
        .build_action(&action.uses, rendered.inputs)
        .map_err(anyhow::Error::from)
        .and_then(|action| {
            action.execute(&ActionContext {
                workspace_id,
                source_id,
                item,
            })
        });
    let (success, stdout, stderr, message) = match run {
        Ok(run) => (run.success, run.stdout, run.stderr, run.message),
        Err(error) => (
            false,
            String::new(),
            String::new(),
            Some(format!("{error:#}")),
        ),
    };
    ActionAttempt {
        ts: Utc::now().to_rfc3339(),
        source_id: source_id.to_string(),
        item_id: item.id.clone(),
        source_action_index: idx,
        uses: action.uses.clone(),
        rendered_action_hash: hash,
        success,
        stdout,
        stderr,
        message,
    }
}

/// Parse an interval string accepted by `watch`.
pub fn parse_duration(s: &str) -> Result<Duration> {
    let secs = s.strip_suffix('s').unwrap_or(s).parse()?;
    Ok(Duration::from_secs(secs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::{ColorChoice, Verbosity};
    use agentboard_core::{
        registry::{Action, ActionDefinition, RuntimeResult},
        ActionRun,
    };
    use schemars::JsonSchema;
    use serde::Deserialize;
    use serde_json::json;
    use std::{collections::BTreeMap, fs, future};

    #[derive(Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields)]
    struct FailingActionConfig {
        message: String,
    }

    struct FailingActionDefinition;

    struct FailingAction {
        message: String,
    }

    impl ActionDefinition for FailingActionDefinition {
        const ID: &'static str = "test/failing";
        type Config = FailingActionConfig;
        type Runtime = FailingAction;

        fn build(config: Self::Config) -> RuntimeResult<Self::Runtime> {
            Ok(FailingAction {
                message: config.message,
            })
        }
    }

    impl Action for FailingAction {
        fn execute(&self, _context: &ActionContext<'_>) -> RuntimeResult<ActionRun> {
            anyhow::bail!(self.message.clone())
        }
    }

    #[test]
    fn registered_action_errors_become_failed_item_attempts() {
        let mut registry = Registry::new();
        registry.add_action::<FailingActionDefinition>().unwrap();
        let item = Item {
            id: "AB-1".into(),
            reference_id: "AB-1".into(),
            title: "Fail locally".into(),
            status: "ready".into(),
            url: "https://example.test/AB-1".into(),
            source_id: "source".into(),
            source_kind: "test".into(),
            raw: json!({}),
        };
        let action = ActionConfig {
            uses: FailingActionDefinition::ID.into(),
            inputs: BTreeMap::from([("message".into(), "expected failure".into())]),
        };
        let rendered = RenderedAction {
            inputs: action.inputs.clone(),
            hash: "rendered-hash".into(),
        };

        let attempt = execute_action_attempt(
            &registry,
            "workspace",
            "source",
            &item,
            0,
            &action,
            rendered,
        );

        assert!(!attempt.success);
        assert_eq!(attempt.rendered_action_hash, "rendered-hash");
        assert_eq!(attempt.message.as_deref(), Some("expected failure"));
    }

    #[tokio::test(start_paused = true)]
    async fn watch_wait_counts_down_against_one_absolute_deadline() {
        let dir = tempfile::tempdir().unwrap();
        let human = dir.path().join("human.txt");
        let output = Output::with_terminal_file_writer(
            Verbosity::Normal,
            ColorChoice::Never,
            true,
            &human,
            None,
        )
        .unwrap();
        let task_output = output.clone();
        let task = tokio::spawn(async move {
            wait_for_next_cycle(
                "work",
                1,
                Duration::from_secs(3),
                &task_output,
                future::pending(),
            )
            .await
            .unwrap()
        });

        tokio::task::yield_now().await;
        assert!(fs::read_to_string(&human).unwrap().ends_with("3s"));

        tokio::time::advance(Duration::from_secs(1)).await;
        tokio::task::yield_now().await;
        assert!(fs::read_to_string(&human).unwrap().ends_with("2s"));

        tokio::time::advance(Duration::from_secs(1)).await;
        tokio::task::yield_now().await;
        assert!(fs::read_to_string(&human).unwrap().ends_with("1s"));

        tokio::time::advance(Duration::from_secs(1)).await;
        assert_eq!(task.await.unwrap(), WaitOutcome::Elapsed);
        let text = fs::read_to_string(human).unwrap();
        assert!(!text.contains("0s"));
        assert!(text.ends_with("\r\x1b[2K"));
    }

    #[tokio::test]
    async fn interrupted_watch_wait_clears_the_transient_line() {
        let dir = tempfile::tempdir().unwrap();
        let human = dir.path().join("human.txt");
        let output = Output::with_terminal_file_writer(
            Verbosity::Normal,
            ColorChoice::Never,
            true,
            &human,
            None,
        )
        .unwrap();

        let outcome = wait_for_next_cycle(
            "work",
            4,
            Duration::from_secs(60),
            &output,
            future::ready(()),
        )
        .await
        .unwrap();

        assert_eq!(outcome, WaitOutcome::Interrupted);
        assert!(fs::read_to_string(human).unwrap().ends_with("\r\x1b[2K"));
    }
}

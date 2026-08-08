use std::{
    future::Future,
    sync::Arc,
    time::{Duration, Instant},
};

use agentboard_core::{
    model::{ActionAttempt, ActionConfig, ActionOutcome, Item, Workspace, WorkspaceSource},
    registry::{ActionContext, Registry, SourceContext},
    CancellationToken,
};
use anyhow::{bail, Result};
use chrono::Utc;
use serde_json::json;

use crate::{
    output::Output,
    store::{acquire_lock, action_key, append_action, successful_actions},
    template::{render_action, ActionTemplateContext},
};

#[derive(Debug, Default)]
struct RunSummary {
    items: usize,
    attempted: usize,
    skipped: usize,
    succeeded: usize,
    failed: usize,
}

#[derive(Debug)]
pub struct InvocationCancelled;

impl std::fmt::Display for InvocationCancelled {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("invocation cancelled")
    }
}

impl std::error::Error for InvocationCancelled {}

pub fn is_cancelled(error: &anyhow::Error) -> bool {
    error.downcast_ref::<InvocationCancelled>().is_some()
}

fn cancelled() -> anyhow::Error {
    InvocationCancelled.into()
}

fn stop_if_cancelled(token: &CancellationToken) -> Result<()> {
    if token.is_cancelled() {
        Err(cancelled())
    } else {
        Ok(())
    }
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
    cancellation: CancellationToken,
) -> Result<()> {
    stop_if_cancelled(&cancellation)?;
    let _lock = if dry_run {
        None
    } else {
        Some(acquire_lock(ws)?)
    };
    run_sources(ws, registry, dry_run, output, cancellation).await
}

/// Repeatedly execute one Workspace Run until Ctrl-C.
pub async fn run_watch(
    ws: Workspace,
    registry: Arc<Registry>,
    dry_run: bool,
    delay: Duration,
    output: &Output,
    cancellation: CancellationToken,
) -> Result<()> {
    stop_if_cancelled(&cancellation)?;
    let _lock = if dry_run {
        None
    } else {
        Some(acquire_lock(&ws)?)
    };
    let run_workspace = ws.clone();
    let cycle_cancellation = cancellation.clone();
    schedule_cycles("run", &ws.id, delay, output, cancellation, move |_| {
        let ws = run_workspace.clone();
        let registry = Arc::clone(&registry);
        let cancellation = cycle_cancellation.clone();
        async move { run_sources(&ws, registry, dry_run, output, cancellation).await }
    })
    .await
}

async fn schedule_cycles<F, Fut>(
    command: &str,
    workspace: &str,
    delay: Duration,
    output: &Output,
    cancellation: CancellationToken,
    cycle: F,
) -> Result<()>
where
    F: FnMut(u64) -> Fut,
    Fut: Future<Output = Result<()>>,
{
    schedule_watch(
        command,
        workspace,
        "cycle",
        delay,
        output,
        cancellation,
        cycle,
    )
    .await
}

/// Repeatedly refresh a read-only view until Ctrl-C.
pub async fn schedule_refreshes<F, Fut>(
    command: &str,
    workspace: &str,
    delay: Duration,
    output: &Output,
    cancellation: CancellationToken,
    refresh: F,
) -> Result<()>
where
    F: FnMut(u64) -> Fut,
    Fut: Future<Output = Result<()>>,
{
    schedule_watch(
        command,
        workspace,
        "refresh",
        delay,
        output,
        cancellation,
        refresh,
    )
    .await
}

async fn schedule_watch<F, Fut>(
    command: &str,
    workspace: &str,
    unit: &str,
    delay: Duration,
    output: &Output,
    cancellation: CancellationToken,
    mut operation: F,
) -> Result<()>
where
    F: FnMut(u64) -> Fut,
    Fut: Future<Output = Result<()>>,
{
    let mut cycle_number = 1_u64;
    loop {
        stop_if_cancelled(&cancellation)?;
        output.info(
            &format!("{command}.watch.{unit}.start"),
            &format!("{command} {workspace} {unit} {cycle_number} starting"),
            watch_metadata(workspace, unit, cycle_number, None, None),
        )?;
        match operation(cycle_number).await {
            Ok(()) => output.success(
                &format!("{command}.watch.{unit}.complete"),
                &format!("{command} {workspace} {unit} {cycle_number} complete"),
                watch_metadata(workspace, unit, cycle_number, Some("pass"), None),
            )?,
            Err(error) if is_cancelled(&error) => {
                output.info(
                    &format!("{command}.watch.{unit}.cancelled"),
                    &format!("{command} {workspace} {unit} {cycle_number} cancelled"),
                    watch_metadata(workspace, unit, cycle_number, Some("cancelled"), None),
                )?;
                return Err(error);
            }
            Err(error) => output.error(
                &format!("{command}.watch.{unit}.failed"),
                &format!("{command} {workspace} {unit} {cycle_number} failed: {error:#}"),
                watch_metadata(
                    workspace,
                    unit,
                    cycle_number,
                    Some("fail"),
                    Some(format!("{error:#}")),
                ),
            )?,
        }
        match wait_for_watch(
            command,
            workspace,
            unit,
            cycle_number,
            delay,
            output,
            async {
                cancellation.cancelled().await;
            },
        )
        .await?
        {
            WaitOutcome::Elapsed => cycle_number += 1,
            WaitOutcome::Interrupted => {
                output.success(
                    &format!("{command}.watch.stop"),
                    &format!("{command} {workspace} stopped by cancellation"),
                    watch_metadata(workspace, unit, cycle_number, Some("cancelled"), None),
                )?;
                return Err(cancelled());
            }
        }
    }
}

fn watch_metadata(
    workspace: &str,
    unit: &str,
    number: u64,
    outcome: Option<&str>,
    error: Option<String>,
) -> serde_json::Value {
    let mut metadata = serde_json::Map::new();
    metadata.insert("workspace".into(), json!(workspace));
    metadata.insert(unit.into(), json!(number));
    if let Some(outcome) = outcome {
        metadata.insert("outcome".into(), json!(outcome));
    }
    if let Some(error) = error {
        metadata.insert("error".into(), json!(error));
    }
    serde_json::Value::Object(metadata)
}

#[cfg(test)]
async fn wait_for_next_cycle<F>(
    command: &str,
    workspace: &str,
    cycle: u64,
    delay: Duration,
    output: &Output,
    stop: F,
) -> Result<WaitOutcome>
where
    F: Future<Output = ()>,
{
    wait_for_watch(command, workspace, "cycle", cycle, delay, output, stop).await
}

async fn wait_for_watch<F>(
    command: &str,
    workspace: &str,
    unit: &str,
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
    let label = if unit == "cycle" { "Run" } else { "refresh" };
    let mut metadata = watch_metadata(workspace, unit, cycle, None, None);
    metadata["delay_seconds"] = json!(delay.as_secs());
    output.transient_info(
        &format!("{command}.watch.wait"),
        &format!("{command} {workspace} next {label} in {}s", delay.as_secs()),
        metadata,
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
                    output.update_transient(&format!("{command} {workspace} next Run in {seconds}s"))?;
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
    cancellation: CancellationToken,
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
        if cancellation.is_cancelled() {
            break;
        }
        let ws = Arc::clone(&ws);
        let registry = Arc::clone(&registry);
        let output = output.clone();
        let run_id = run_id.clone();
        let cancellation = cancellation.clone();
        handles.push(tokio::spawn(async move {
            run_source_with_cancellation(
                &ws,
                &source,
                &registry,
                dry_run,
                &run_id,
                &output,
                cancellation,
            )
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
            Err((source_id, err)) if is_cancelled(&err) => {
                output.info(
                    "source.cancelled",
                    &format!("source {source_id} cancelled"),
                    json!({"workspace": ws.id, "run": run_id, "source": source_id, "outcome": "cancelled"}),
                )?;
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
    if cancellation.is_cancelled() {
        output.info(
            "run.cancelled",
            &format!("run {} cancellation observed between work units", ws.id),
            json!({"workspace": ws.id, "run": run_id, "outcome": "cancelled"}),
        )?;
        return Err(cancelled());
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
#[cfg(test)]
async fn run_source(
    ws: &Workspace,
    source: &WorkspaceSource,
    registry: &Registry,
    dry_run: bool,
    run_id: &str,
    output: &Output,
) -> Result<RunSummary> {
    run_source_with_cancellation(
        ws,
        source,
        registry,
        dry_run,
        run_id,
        output,
        CancellationToken::new(),
    )
    .await
}

async fn run_source_with_cancellation(
    ws: &Workspace,
    source: &WorkspaceSource,
    registry: &Registry,
    dry_run: bool,
    run_id: &str,
    output: &Output,
    cancellation: CancellationToken,
) -> Result<RunSummary> {
    run_source_with_store_hook(
        ws,
        source,
        registry,
        dry_run,
        run_id,
        output,
        cancellation,
        None,
    )
    .await
}

#[cfg(test)]
async fn run_source_with_store_publication_cancellation(
    ws: &Workspace,
    source: &WorkspaceSource,
    registry: &Registry,
    dry_run: bool,
    run_id: &str,
    output: &Output,
    cancellation: CancellationToken,
) -> Result<RunSummary> {
    run_source_with_store_hook(
        ws,
        source,
        registry,
        dry_run,
        run_id,
        output,
        cancellation,
        Some(|cancellation| cancellation.cancel()),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn run_source_with_store_hook(
    ws: &Workspace,
    source: &WorkspaceSource,
    registry: &Registry,
    dry_run: bool,
    run_id: &str,
    output: &Output,
    cancellation: CancellationToken,
    after_store_publication: Option<fn(&CancellationToken)>,
) -> Result<RunSummary> {
    stop_if_cancelled(&cancellation)?;
    let source_id = &source.configured.id;
    let started = Instant::now();
    output.detail(
        "source.start",
        &format!("source {source_id} collecting"),
        json!({"workspace": ws.id, "run": run_id, "source": source_id}),
    )?;
    let context = SourceContext {
        source_id,
        cancellation: cancellation.clone(),
    };
    let mut items = source
        .built
        .runtime()
        .collect(&context)
        .await
        .map_err(|error| {
            if cancellation.is_cancelled() {
                cancelled()
            } else {
                error
            }
        })?
        .items;
    stop_if_cancelled(&cancellation)?;
    items.sort_by(|a, b| a.id.cmp(&b.id));
    stop_if_cancelled(&cancellation)?;
    if !dry_run {
        let ws = ws.clone();
        let source = source.clone();
        let items = items.clone();
        let append_cancellation = cancellation.clone();
        tokio::task::spawn_blocking(move || {
            crate::store::append_items_with_cancellation(&ws, &source, &items, &append_cancellation)
        })
        .await??;
        if let Some(after_store_publication) = after_store_publication {
            after_store_publication(&cancellation);
        }
    }
    stop_if_cancelled(&cancellation)?;
    let successes = successful_actions(ws, source)?;
    let mut summary = RunSummary {
        items: items.len(),
        ..Default::default()
    };
    for item in items {
        stop_if_cancelled(&cancellation)?;
        let mut actions = ActionTemplateContext::new();
        for (idx, action) in source.configured.actions.iter().enumerate() {
            stop_if_cancelled(&cancellation)?;
            let rendered = match render_action(ws, source, &item, idx, action, &actions) {
                Ok(rendered) => rendered,
                Err(error) => {
                    if cancellation.is_cancelled() {
                        return Err(cancelled());
                    }
                    summary.attempted += 1;
                    summary.failed += 1;
                    let message = format!("render action inputs: {error:#}");
                    if !dry_run {
                        append_action(
                            ws,
                            source,
                            &failed_action_attempt(source_id, &item, idx, action, message.clone()),
                        )?;
                    }
                    output.error(
                        "action.failed",
                        &format!(
                            "{source_id} {} action#{idx} {} failed: {message}",
                            item.id, action.uses
                        ),
                        json!({"workspace": ws.id, "run": run_id, "source": source_id, "item": item.id, "action_index": idx, "uses": action.uses, "error": message}),
                    )?;
                    break;
                }
            };
            stop_if_cancelled(&cancellation)?;
            if let Some(id) = &action.id {
                actions.insert(id.clone(), rendered.inputs.clone());
            }
            let key = action_key(source_id, &item.id, idx, &rendered.hash);
            let built_action = registry.build_action(&action.uses, rendered.inputs.clone());
            stop_if_cancelled(&cancellation)?;
            let context = ActionContext {
                workspace_id: &ws.id,
                source_id,
                item: &item,
                cancellation: cancellation.clone(),
            };
            if successes.contains(&key)
                && built_action
                    .as_ref()
                    .is_ok_and(|action| action.cached_success_is_valid(&context))
            {
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
            let action_context = context.cancellation.clone();
            let workspace_id = ws.id.clone();
            let source_id_owned = source_id.clone();
            let item_owned = item.clone();
            let action_owned = action.clone();
            let rendered_hash = rendered.hash;
            let attempt = tokio::task::spawn_blocking(move || {
                let context = ActionContext {
                    workspace_id: &workspace_id,
                    source_id: &source_id_owned,
                    item: &item_owned,
                    cancellation: action_context,
                };
                execute_action_attempt(
                    built_action,
                    &context,
                    &source_id_owned,
                    &item_owned,
                    idx,
                    &action_owned,
                    rendered_hash,
                )
            })
            .await?;
            append_action(ws, source, &attempt)?;
            if attempt.outcome == ActionOutcome::Cancelled {
                output.info(
                    "action.cancelled",
                    &format!("{source_id} {} action#{idx} cancelled", item.id),
                    json!({"workspace": ws.id, "run": run_id, "source": source_id, "item": item.id, "action_index": idx, "uses": action.uses, "outcome": "cancelled"}),
                )?;
                return Err(cancelled());
            }
            if attempt.outcome == ActionOutcome::Success {
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
    stop_if_cancelled(&cancellation)?;
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

fn failed_action_attempt(
    source_id: &str,
    item: &Item,
    idx: usize,
    action: &ActionConfig,
    message: String,
) -> ActionAttempt {
    ActionAttempt {
        ts: Utc::now().to_rfc3339(),
        source_id: source_id.to_string(),
        item_id: item.id.clone(),
        source_action_index: idx,
        uses: action.uses.clone(),
        rendered_action_hash: String::new(),
        outcome: ActionOutcome::Failure,
        stdout: String::new(),
        stderr: String::new(),
        message: Some(message),
    }
}

fn execute_action_attempt(
    action_runtime: Result<
        Box<dyn agentboard_core::registry::Action>,
        agentboard_core::registry::RegistryError,
    >,
    context: &ActionContext<'_>,
    source_id: &str,
    item: &Item,
    idx: usize,
    action: &ActionConfig,
    hash: String,
) -> ActionAttempt {
    let run = action_runtime
        .map_err(anyhow::Error::from)
        .and_then(|action| action.execute(context));
    let (outcome, stdout, stderr, message) = match run {
        Ok(run) => (run.outcome, run.stdout, run.stderr, run.message),
        Err(error) => (
            if context.cancellation.is_cancelled() {
                ActionOutcome::Cancelled
            } else {
                ActionOutcome::Failure
            },
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
        outcome,
        stdout,
        stderr,
        message,
    }
}

/// Parse an interval string accepted by `run --watch`.
pub fn parse_duration(s: &str) -> Result<Duration> {
    let secs: u64 = s.strip_suffix('s').unwrap_or(s).parse()?;
    if secs == 0 {
        bail!("interval must be greater than zero");
    }
    Ok(Duration::from_secs(secs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{actions_path, parse_workspace, store_root},
        output::{ColorChoice, Verbosity},
        store::latest_items,
    };
    use agentboard_core::{
        registry::{
            Action, ActionDefinition, RuntimeResult, Source, SourceCollection, SourceDefinition,
            SourceFuture,
        },
        ActionRun, RenderedAction,
    };
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use std::{
        collections::BTreeMap,
        fs, future,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Mutex,
        },
        time::{SystemTime, UNIX_EPOCH},
    };

    #[derive(Deserialize, Serialize, JsonSchema)]
    #[serde(deny_unknown_fields)]
    struct ItemSourceConfig {}

    struct ItemSourceDefinition;

    struct ItemSource;

    static COLLECTION_PROBE_CALLS: AtomicUsize = AtomicUsize::new(0);
    static CANCELLING_SOURCE_COLLECTIONS: AtomicUsize = AtomicUsize::new(0);
    static TEST_ACTION_INVOCATIONS: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

    fn action_invocation_count(workspace_id: &str, message: &str) -> usize {
        TEST_ACTION_INVOCATIONS
            .lock()
            .unwrap()
            .iter()
            .filter(|(invocation_workspace, invocation_message)| {
                invocation_workspace == workspace_id && invocation_message == message
            })
            .count()
    }

    struct CollectionProbeSourceDefinition;

    struct CollectionProbeSource;

    impl SourceDefinition for CollectionProbeSourceDefinition {
        const ID: &'static str = "test/collection-probe";
        type Config = ItemSourceConfig;
        type Runtime = CollectionProbeSource;

        fn build(_config: Self::Config) -> RuntimeResult<Self::Runtime> {
            Ok(CollectionProbeSource)
        }
    }

    impl Source for CollectionProbeSource {
        fn collect<'a>(&'a self, context: &'a SourceContext<'a>) -> SourceFuture<'a> {
            Box::pin(async move {
                COLLECTION_PROBE_CALLS.fetch_add(1, Ordering::SeqCst);
                Ok(SourceCollection {
                    items: vec![test_item(context.source_id, "probe", json!({}))],
                    available: None,
                    limit: 1,
                })
            })
        }

        fn item_bucket_identity(&self) -> String {
            "collection-probe".into()
        }
    }

    struct CancellingSourceDefinition;

    struct CancellingSource;

    impl SourceDefinition for CancellingSourceDefinition {
        const ID: &'static str = "test/cancelling-source";
        type Config = ItemSourceConfig;
        type Runtime = CancellingSource;

        fn build(_config: Self::Config) -> RuntimeResult<Self::Runtime> {
            Ok(CancellingSource)
        }
    }

    impl Source for CancellingSource {
        fn collect<'a>(&'a self, context: &'a SourceContext<'a>) -> SourceFuture<'a> {
            Box::pin(async move {
                CANCELLING_SOURCE_COLLECTIONS.fetch_add(1, Ordering::SeqCst);
                context.cancellation.cancel();
                Ok(SourceCollection {
                    items: vec![test_item(context.source_id, "cancelled", json!({}))],
                    available: None,
                    limit: 1,
                })
            })
        }

        fn item_bucket_identity(&self) -> String {
            "cancelling-source".into()
        }
    }

    impl SourceDefinition for ItemSourceDefinition {
        const ID: &'static str = "test/items";
        type Config = ItemSourceConfig;
        type Runtime = ItemSource;

        fn build(_config: Self::Config) -> RuntimeResult<Self::Runtime> {
            Ok(ItemSource)
        }
    }

    impl Source for ItemSource {
        fn collect<'a>(&'a self, context: &'a SourceContext<'a>) -> SourceFuture<'a> {
            Box::pin(async move {
                Ok(SourceCollection {
                    items: vec![
                        test_item(context.source_id, "bad", json!({"value": {}})),
                        test_item(context.source_id, "good", json!({"value": 1})),
                    ],
                    available: None,
                    limit: 2,
                })
            })
        }

        fn item_bucket_identity(&self) -> String {
            "items".into()
        }
    }

    #[derive(Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields)]
    struct TestActionConfig {
        message: String,
        #[serde(default)]
        outcome: String,
    }

    struct TestActionDefinition;

    struct TestAction {
        message: String,
        outcome: String,
    }

    impl ActionDefinition for TestActionDefinition {
        const ID: &'static str = "test/action";
        type Config = TestActionConfig;
        type Runtime = TestAction;

        fn build(config: Self::Config) -> RuntimeResult<Self::Runtime> {
            Ok(TestAction {
                message: config.message,
                outcome: config.outcome,
            })
        }
    }

    impl Action for TestAction {
        fn cached_success_is_valid(&self, _context: &ActionContext<'_>) -> bool {
            self.outcome != "stale"
        }

        fn execute(&self, context: &ActionContext<'_>) -> RuntimeResult<ActionRun> {
            TEST_ACTION_INVOCATIONS
                .lock()
                .unwrap()
                .push((context.workspace_id.into(), self.message.clone()));
            if self.outcome == "fail" {
                anyhow::bail!(self.message.clone());
            }
            if self.outcome == "cancel" {
                context.cancellation.cancel();
            }
            Ok(ActionRun {
                outcome: ActionOutcome::Success,
                stdout: self.message.clone(),
                stderr: String::new(),
                message: None,
            })
        }
    }

    #[test]
    fn registered_action_errors_become_failed_item_attempts() {
        let mut registry = Registry::new();
        registry.add_action::<TestActionDefinition>().unwrap();
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
            id: None,
            uses: TestActionDefinition::ID.into(),
            inputs: BTreeMap::from([
                ("message".into(), "expected failure".into()),
                ("outcome".into(), "fail".into()),
            ]),
        };
        let rendered = RenderedAction {
            inputs: action.inputs.clone(),
            hash: "rendered-hash".into(),
        };

        let attempt = execute_action_attempt(
            registry.build_action(&action.uses, rendered.inputs),
            &ActionContext {
                workspace_id: "workspace",
                source_id: "source",
                item: &item,
                cancellation: CancellationToken::new(),
            },
            "source",
            &item,
            0,
            &action,
            rendered.hash,
        );

        assert_eq!(attempt.outcome, ActionOutcome::Failure);
        assert_eq!(attempt.rendered_action_hash, "rendered-hash");
        assert_eq!(attempt.message.as_deref(), Some("expected failure"));
    }

    #[tokio::test]
    async fn render_errors_are_item_scoped_persisted_and_dry_run_safe() {
        let mut registry = Registry::new();
        registry.add_source::<ItemSourceDefinition>().unwrap();
        registry.add_action::<TestActionDefinition>().unwrap();
        let parsed = parse_workspace(
            r#"
                [[sources]]
                id = "source"
                [sources.source]
                kind = "test/items"

                [[sources.actions]]
                uses = "test/action"
                [sources.actions.with]
                message = "{{ item.raw.value + 1 }}"

                [[sources.actions]]
                uses = "test/action"
                [sources.actions.with]
                message = "later"
            "#,
            &registry,
        )
        .unwrap();
        let ws = Workspace {
            id: format!(
                "runtime-render-error-{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ),
            path: "work.toml".into(),
            sources: parsed.sources,
        };
        let dir = tempfile::tempdir().unwrap();
        let output = Output::with_terminal_file_writer(
            Verbosity::Quiet,
            ColorChoice::Never,
            false,
            &dir.path().join("human.txt"),
            None,
        )
        .unwrap();

        let dry_summary = run_source(&ws, &ws.sources[0], &registry, true, "dry-run", &output)
            .await
            .unwrap();
        assert_eq!(dry_summary.attempted, 3);
        assert_eq!(dry_summary.succeeded, 2);
        assert_eq!(dry_summary.failed, 1);
        assert!(!store_root(&ws).exists());

        let summary = run_source(&ws, &ws.sources[0], &registry, false, "run", &output)
            .await
            .unwrap();
        assert_eq!(summary.attempted, 3);
        assert_eq!(summary.succeeded, 2);
        assert_eq!(summary.failed, 1);

        let attempts = fs::read_to_string(actions_path(&ws, &ws.sources[0]))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<ActionAttempt>(line).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(attempts.len(), 3);
        assert_eq!(attempts[0].item_id, "bad");
        assert_eq!(attempts[0].source_action_index, 0);
        assert!(attempts[0].rendered_action_hash.is_empty());
        assert_eq!(attempts[0].outcome, ActionOutcome::Failure);
        assert!(attempts[0]
            .message
            .as_deref()
            .unwrap()
            .contains("render action inputs"));
        assert_eq!(attempts[1].item_id, "good");
        assert_eq!(attempts[1].source_action_index, 0);
        assert_eq!(attempts[1].outcome, ActionOutcome::Success);
        assert_eq!(attempts[2].item_id, "good");
        assert_eq!(attempts[2].source_action_index, 1);
        assert_eq!(attempts[2].outcome, ActionOutcome::Success);

        fs::remove_dir_all(store_root(&ws)).unwrap();
    }

    #[tokio::test]
    async fn named_action_inputs_flow_through_dry_run_execution_and_stored_success() {
        let mut registry = Registry::new();
        registry.add_source::<ItemSourceDefinition>().unwrap();
        registry.add_action::<TestActionDefinition>().unwrap();
        let parsed = parse_workspace(
            r#"
                [[sources]]
                id = "source"
                [sources.source]
                kind = "test/items"

                [[sources.actions]]
                id = "first"
                uses = "test/action"
                [sources.actions.with]
                message = "first-{{ item.id }}"

                [[sources.actions]]
                uses = "test/action"
                [sources.actions.with]
                message = "{{ actions.first.inputs.message }}"
            "#,
            &registry,
        )
        .unwrap();
        let ws = Workspace {
            id: format!(
                "runtime-action-context-{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ),
            path: "work.toml".into(),
            sources: parsed.sources,
        };
        let dir = tempfile::tempdir().unwrap();
        let output = Output::with_terminal_file_writer(
            Verbosity::Quiet,
            ColorChoice::Never,
            false,
            &dir.path().join("human.txt"),
            None,
        )
        .unwrap();

        let dry_summary = run_source(&ws, &ws.sources[0], &registry, true, "dry-run", &output)
            .await
            .unwrap();
        assert_eq!(dry_summary.attempted, 4);
        assert_eq!(dry_summary.succeeded, 4);
        assert!(!store_root(&ws).exists());

        let summary = run_source(&ws, &ws.sources[0], &registry, false, "run", &output)
            .await
            .unwrap();
        assert_eq!(summary.attempted, 4);
        assert_eq!(summary.succeeded, 4);

        let attempts = fs::read_to_string(actions_path(&ws, &ws.sources[0]))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<ActionAttempt>(line).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(attempts.len(), 4);
        assert_eq!(attempts[1].stdout, "first-bad");
        assert_eq!(attempts[3].stdout, "first-good");

        let skipped = run_source(
            &ws,
            &ws.sources[0],
            &registry,
            false,
            "stored-success",
            &output,
        )
        .await
        .unwrap();
        assert_eq!(skipped.attempted, 0);
        assert_eq!(skipped.skipped, 4);
        assert_eq!(skipped.failed, 0);

        fs::remove_dir_all(store_root(&ws)).unwrap();
    }

    #[tokio::test]
    async fn invalid_cached_success_reexecutes_action() {
        let mut registry = Registry::new();
        registry.add_source::<ItemSourceDefinition>().unwrap();
        registry.add_action::<TestActionDefinition>().unwrap();
        let parsed = parse_workspace(
            r#"
                [[sources]]
                id = "source"
                [sources.source]
                kind = "test/items"

                [[sources.actions]]
                uses = "test/action"
                [sources.actions.with]
                message = "reconcile-{{ item.id }}"
                outcome = "stale"
            "#,
            &registry,
        )
        .unwrap();
        let ws = Workspace {
            id: format!(
                "runtime-invalid-cache-{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ),
            path: "work.toml".into(),
            sources: parsed.sources,
        };
        let dir = tempfile::tempdir().unwrap();
        let output = Output::with_terminal_file_writer(
            Verbosity::Quiet,
            ColorChoice::Never,
            false,
            &dir.path().join("human.txt"),
            None,
        )
        .unwrap();

        let first = run_source(&ws, &ws.sources[0], &registry, false, "first", &output)
            .await
            .unwrap();
        assert_eq!(first.attempted, 2);
        assert_eq!(first.succeeded, 2);

        let reconciled = run_source(&ws, &ws.sources[0], &registry, false, "reconciled", &output)
            .await
            .unwrap();
        assert_eq!(reconciled.attempted, 2);
        assert_eq!(reconciled.skipped, 0);
        assert_eq!(reconciled.succeeded, 2);

        let attempts = fs::read_to_string(actions_path(&ws, &ws.sources[0]))
            .unwrap()
            .lines()
            .count();
        assert_eq!(attempts, 4);

        fs::remove_dir_all(store_root(&ws)).unwrap();
    }

    #[tokio::test]
    async fn missing_named_action_input_is_an_item_scoped_render_failure() {
        let mut registry = Registry::new();
        registry.add_source::<ItemSourceDefinition>().unwrap();
        registry.add_action::<TestActionDefinition>().unwrap();
        let parsed = parse_workspace(
            r#"
                [[sources]]
                id = "source"
                [sources.source]
                kind = "test/items"

                [[sources.actions]]
                id = "first"
                uses = "test/action"
                [sources.actions.with]
                message = "first-{{ item.id }}"

                [[sources.actions]]
                uses = "test/action"
                [sources.actions.with]
                message = "{{ actions.first.inputs.missing }}"

                [[sources.actions]]
                uses = "test/action"
                [sources.actions.with]
                message = "must not run"
            "#,
            &registry,
        )
        .unwrap();
        let ws = Workspace {
            id: format!(
                "runtime-missing-action-input-{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ),
            path: "work.toml".into(),
            sources: parsed.sources,
        };
        let dir = tempfile::tempdir().unwrap();
        let output = Output::with_terminal_file_writer(
            Verbosity::Quiet,
            ColorChoice::Never,
            false,
            &dir.path().join("human.txt"),
            None,
        )
        .unwrap();

        let summary = run_source(&ws, &ws.sources[0], &registry, false, "run", &output)
            .await
            .unwrap();
        assert_eq!(summary.attempted, 4);
        assert_eq!(summary.succeeded, 2);
        assert_eq!(summary.failed, 2);

        let attempts = fs::read_to_string(actions_path(&ws, &ws.sources[0]))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<ActionAttempt>(line).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(attempts.len(), 4);
        assert_eq!(
            attempts
                .iter()
                .map(|attempt| (attempt.source_action_index, attempt.outcome))
                .collect::<Vec<_>>(),
            [
                (0, ActionOutcome::Success),
                (1, ActionOutcome::Failure),
                (0, ActionOutcome::Success),
                (1, ActionOutcome::Failure),
            ]
        );
        assert!(attempts
            .iter()
            .filter(|attempt| attempt.source_action_index == 1)
            .all(
                |attempt| attempt.message.as_deref().is_some_and(|message| message
                    .contains("undefined Action template reference actions.first.inputs.missing"))
            ));

        fs::remove_dir_all(store_root(&ws)).unwrap();
    }

    #[tokio::test]
    async fn failed_named_action_stops_later_actions_for_each_item() {
        let mut registry = Registry::new();
        registry.add_source::<ItemSourceDefinition>().unwrap();
        registry.add_action::<TestActionDefinition>().unwrap();
        let parsed = parse_workspace(
            r#"
                [[sources]]
                id = "source"
                [sources.source]
                kind = "test/items"

                [[sources.actions]]
                id = "first"
                uses = "test/action"
                [sources.actions.with]
                message = "stop"
                outcome = "fail"

                [[sources.actions]]
                uses = "test/action"
                [sources.actions.with]
                message = "{{ actions.first.inputs.message }}"
            "#,
            &registry,
        )
        .unwrap();
        let ws = Workspace {
            id: format!(
                "runtime-action-failure-{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ),
            path: "work.toml".into(),
            sources: parsed.sources,
        };
        let dir = tempfile::tempdir().unwrap();
        let output = Output::with_terminal_file_writer(
            Verbosity::Quiet,
            ColorChoice::Never,
            false,
            &dir.path().join("human.txt"),
            None,
        )
        .unwrap();

        let summary = run_source(&ws, &ws.sources[0], &registry, false, "run", &output)
            .await
            .unwrap();
        assert_eq!(summary.attempted, 2);
        assert_eq!(summary.failed, 2);

        let attempts = fs::read_to_string(actions_path(&ws, &ws.sources[0]))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<ActionAttempt>(line).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(attempts.len(), 2);
        assert!(attempts.iter().all(|attempt| {
            attempt.source_action_index == 0 && attempt.outcome == ActionOutcome::Failure
        }));

        fs::remove_dir_all(store_root(&ws)).unwrap();
    }

    #[tokio::test]
    async fn cancelled_invocation_prevents_new_source_work() {
        COLLECTION_PROBE_CALLS.store(0, Ordering::SeqCst);
        let mut registry = Registry::new();
        registry
            .add_source::<CollectionProbeSourceDefinition>()
            .unwrap();
        let parsed = parse_workspace(
            r#"
                [[sources]]
                id = "source"
                [sources.source]
                kind = "test/collection-probe"
            "#,
            &registry,
        )
        .unwrap();
        let ws = Workspace {
            id: "cancelled-source".into(),
            path: "work.toml".into(),
            sources: parsed.sources,
        };
        let dir = tempfile::tempdir().unwrap();
        let output = Output::with_terminal_file_writer(
            Verbosity::Quiet,
            ColorChoice::Never,
            false,
            &dir.path().join("human.txt"),
            None,
        )
        .unwrap();
        let cancellation = CancellationToken::new();
        cancellation.cancel();

        let error = run_source_with_cancellation(
            &ws,
            &ws.sources[0],
            &registry,
            false,
            "cancelled",
            &output,
            cancellation,
        )
        .await
        .unwrap_err();

        assert!(is_cancelled(&error));
        assert_eq!(COLLECTION_PROBE_CALLS.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn cancelled_collection_stops_before_store_and_reports_cancellation() {
        CANCELLING_SOURCE_COLLECTIONS.store(0, Ordering::SeqCst);
        let mut registry = Registry::new();
        registry.add_source::<CancellingSourceDefinition>().unwrap();
        let parsed = parse_workspace(
            r#"
                [[sources]]
                id = "source"
                [sources.source]
                kind = "test/cancelling-source"
            "#,
            &registry,
        )
        .unwrap();
        let ws = Workspace {
            id: format!(
                "cancelled-collection-{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ),
            path: "work.toml".into(),
            sources: parsed.sources,
        };
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().join("events.jsonl");
        let output = Output::with_terminal_file_writer(
            Verbosity::Quiet,
            ColorChoice::Never,
            false,
            &dir.path().join("human.txt"),
            Some(&events),
        )
        .unwrap();
        let cancellation = CancellationToken::new();

        let error = run_sources(&ws, Arc::new(registry), false, &output, cancellation)
            .await
            .unwrap_err();

        assert!(is_cancelled(&error));
        assert_eq!(CANCELLING_SOURCE_COLLECTIONS.load(Ordering::SeqCst), 1);
        assert!(!store_root(&ws).exists());
        let events = fs::read_to_string(events).unwrap();
        assert!(events.contains("\"stage\":\"source.cancelled\""));
        assert!(events.contains("\"stage\":\"run.cancelled\""));
        assert!(!events.contains("\"stage\":\"source.failed\""));
    }

    #[tokio::test]
    async fn cancellation_after_store_publication_keeps_snapshot_and_starts_no_actions() {
        let mut registry = Registry::new();
        registry.add_source::<ItemSourceDefinition>().unwrap();
        registry.add_action::<TestActionDefinition>().unwrap();
        let parsed = parse_workspace(
            r#"
                [[sources]]
                id = "source"
                [sources.source]
                kind = "test/items"

                [[sources.actions]]
                uses = "test/action"
                [sources.actions.with]
                message = "must not run"
            "#,
            &registry,
        )
        .unwrap();
        let ws = Workspace {
            id: format!(
                "cancelled-after-store-{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ),
            path: "work.toml".into(),
            sources: parsed.sources,
        };
        let dir = tempfile::tempdir().unwrap();
        let output = Output::with_terminal_file_writer(
            Verbosity::Quiet,
            ColorChoice::Never,
            false,
            &dir.path().join("human.txt"),
            None,
        )
        .unwrap();
        let cancellation = CancellationToken::new();

        let error = run_source_with_store_publication_cancellation(
            &ws,
            &ws.sources[0],
            &registry,
            false,
            "cancelled",
            &output,
            cancellation,
        )
        .await
        .unwrap_err();

        assert!(is_cancelled(&error));
        assert_eq!(action_invocation_count(&ws.id, "must not run"), 0);
        assert!(!actions_path(&ws, &ws.sources[0]).exists());
        assert_eq!(latest_items(&ws).unwrap().len(), 2);
        fs::remove_dir_all(store_root(&ws)).unwrap();
    }

    #[tokio::test]
    async fn cancellation_between_actions_prevents_later_action_work() {
        let mut registry = Registry::new();
        registry.add_source::<ItemSourceDefinition>().unwrap();
        registry.add_action::<TestActionDefinition>().unwrap();
        let parsed = parse_workspace(
            r#"
                [[sources]]
                id = "source"
                [sources.source]
                kind = "test/items"

                [[sources.actions]]
                uses = "test/action"
                [sources.actions.with]
                message = "cancel"
                outcome = "cancel"

                [[sources.actions]]
                uses = "test/action"
                [sources.actions.with]
                message = "must not run"
            "#,
            &registry,
        )
        .unwrap();
        let ws = Workspace {
            id: format!(
                "cancelled-action-{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ),
            path: "work.toml".into(),
            sources: parsed.sources,
        };
        let dir = tempfile::tempdir().unwrap();
        let output = Output::with_terminal_file_writer(
            Verbosity::Quiet,
            ColorChoice::Never,
            false,
            &dir.path().join("human.txt"),
            None,
        )
        .unwrap();

        let error = run_source(&ws, &ws.sources[0], &registry, false, "cancelled", &output)
            .await
            .unwrap_err();

        assert!(is_cancelled(&error));
        assert_eq!(action_invocation_count(&ws.id, "cancel"), 1);
        assert_eq!(action_invocation_count(&ws.id, "must not run"), 0);
        let attempts = fs::read_to_string(actions_path(&ws, &ws.sources[0]))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<ActionAttempt>(line).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].outcome, ActionOutcome::Success);
        assert_eq!(latest_items(&ws).unwrap().len(), 2);
        fs::remove_dir_all(store_root(&ws)).unwrap();
    }

    fn test_item(source_id: &str, id: &str, raw: serde_json::Value) -> Item {
        Item {
            id: id.into(),
            reference_id: id.into(),
            title: id.into(),
            status: "ready".into(),
            url: format!("https://example.test/{id}"),
            source_id: source_id.into(),
            source_kind: ItemSourceDefinition::ID.into(),
            raw,
        }
    }

    #[tokio::test(start_paused = true)]
    async fn scheduler_runs_immediately_and_delays_from_completion() {
        let dir = tempfile::tempdir().unwrap();
        let output = Output::with_terminal_file_writer(
            Verbosity::Quiet,
            ColorChoice::Never,
            false,
            &dir.path().join("human.txt"),
            Some(&dir.path().join("events.jsonl")),
        )
        .unwrap();
        let cancellation = CancellationToken::new();
        let calls = Arc::new(AtomicUsize::new(0));
        let started = Arc::new(Mutex::new(Vec::new()));
        let task_calls = Arc::clone(&calls);
        let task_started = Arc::clone(&started);
        let task_cancellation = cancellation.clone();
        let task = tokio::spawn(async move {
            schedule_cycles(
                "run",
                "work",
                Duration::from_secs(5),
                &output,
                cancellation,
                move |_| {
                    let call = task_calls.fetch_add(1, Ordering::SeqCst) + 1;
                    task_started
                        .lock()
                        .unwrap()
                        .push(tokio::time::Instant::now());
                    let cancellation = task_cancellation.clone();
                    async move {
                        if call == 1 {
                            tokio::time::sleep(Duration::from_secs(2)).await;
                            Ok(())
                        } else {
                            cancellation.cancel();
                            Err(cancelled())
                        }
                    }
                },
            )
            .await
        });

        tokio::task::yield_now().await;
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        tokio::time::advance(Duration::from_secs(2)).await;
        tokio::task::yield_now().await;
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        tokio::time::advance(Duration::from_secs(4)).await;
        tokio::task::yield_now().await;
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        tokio::time::advance(Duration::from_secs(1)).await;
        tokio::task::yield_now().await;
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert!(is_cancelled(&task.await.unwrap().unwrap_err()));

        let started = started.lock().unwrap();
        assert_eq!(started[1] - started[0], Duration::from_secs(7));
        let events = fs::read_to_string(dir.path().join("events.jsonl")).unwrap();
        assert!(events.contains("\"stage\":\"run.watch.cycle.start\""));
        assert!(events.contains("\"stage\":\"run.watch.cycle.complete\""));
    }

    #[tokio::test(start_paused = true)]
    async fn scheduler_reports_failures_retries_and_stops_without_an_extra_cycle() {
        let dir = tempfile::tempdir().unwrap();
        let output = Output::with_terminal_file_writer(
            Verbosity::Quiet,
            ColorChoice::Never,
            false,
            &dir.path().join("human.txt"),
            Some(&dir.path().join("events.jsonl")),
        )
        .unwrap();
        let cancellation = CancellationToken::new();
        let calls = Arc::new(AtomicUsize::new(0));
        let task_calls = Arc::clone(&calls);
        let task_cancellation = cancellation.clone();
        let task = tokio::spawn(async move {
            schedule_cycles(
                "run",
                "work",
                Duration::from_secs(3),
                &output,
                cancellation,
                move |_| {
                    let call = task_calls.fetch_add(1, Ordering::SeqCst) + 1;
                    let cancellation = task_cancellation.clone();
                    async move {
                        if call == 1 {
                            Err(anyhow::anyhow!("cycle failed"))
                        } else {
                            cancellation.cancel();
                            Ok(())
                        }
                    }
                },
            )
            .await
        });

        tokio::task::yield_now().await;
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        tokio::time::advance(Duration::from_secs(3)).await;
        tokio::task::yield_now().await;
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert!(is_cancelled(&task.await.unwrap().unwrap_err()));

        let events = fs::read_to_string(dir.path().join("events.jsonl")).unwrap();
        assert!(events.contains("\"stage\":\"run.watch.cycle.failed\""));
        assert!(events.contains("\"stage\":\"run.watch.cycle.complete\""));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn refresh_scheduler_uses_command_specific_events_and_retries() {
        let dir = tempfile::tempdir().unwrap();
        let output = Output::with_terminal_file_writer(
            Verbosity::Quiet,
            ColorChoice::Never,
            false,
            &dir.path().join("human.txt"),
            Some(&dir.path().join("events.jsonl")),
        )
        .unwrap();
        let cancellation = CancellationToken::new();
        let calls = Arc::new(AtomicUsize::new(0));
        let task_calls = Arc::clone(&calls);
        let task_cancellation = cancellation.clone();
        let task = tokio::spawn(async move {
            schedule_refreshes(
                "list",
                "work",
                Duration::from_secs(1),
                &output,
                cancellation,
                move |_| {
                    let call = task_calls.fetch_add(1, Ordering::SeqCst) + 1;
                    let cancellation = task_cancellation.clone();
                    async move {
                        if call == 1 {
                            Err(anyhow::anyhow!("refresh failed"))
                        } else {
                            cancellation.cancel();
                            Ok(())
                        }
                    }
                },
            )
            .await
        });

        tokio::task::yield_now().await;
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        tokio::time::advance(Duration::from_secs(1)).await;
        tokio::task::yield_now().await;
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert!(is_cancelled(&task.await.unwrap().unwrap_err()));

        let events = fs::read_to_string(dir.path().join("events.jsonl")).unwrap();
        assert!(events.contains(r#""stage":"list.watch.refresh.failed""#));
        assert!(events.contains(r#""stage":"list.watch.refresh.complete""#));
        assert!(events.contains(r#""refresh":1"#));
    }

    #[test]
    fn parse_duration_rejects_zero_and_invalid_values() {
        assert_eq!(parse_duration("5s").unwrap(), Duration::from_secs(5));
        assert_eq!(parse_duration("5").unwrap(), Duration::from_secs(5));
        assert!(parse_duration("0").is_err());
        assert!(parse_duration("nope").is_err());
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
                "run",
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
            "run",
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

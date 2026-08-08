//! Append-only Workspace Store operations and registered diagnostic checks.

use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    hash::{Hash, Hasher},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use agentboard_core::{
    model::{ActionAttempt, ActionOutcome, Item, Workspace, WorkspaceSource},
    registry::{HealthCheckContext, Registry, SourceCollection},
    CancellationToken,
};
use anyhow::{anyhow, bail, Context, Result};
use fs4::{FileExt, TryLockError};
use serde_json::{json, Value};

use crate::{
    config::{actions_path, items_path, source_slug, store_root},
    output::Output,
};

#[derive(Clone, Debug)]
struct StoredItem {
    slug: String,
    item: Item,
    snapshot_key: Option<String>,
    snapshot_id: Option<String>,
}

const SNAPSHOT_KEY_FIELD: &str = "_agentboard_snapshot_key";
const SNAPSHOT_ID_FIELD: &str = "_agentboard_snapshot_id";
static SNAPSHOT_COUNTER: AtomicU64 = AtomicU64::new(0);

struct StoredAction {
    slug: String,
    attempt: ActionAttempt,
}

/// Held workspace run lock. Unlocks when dropped.
pub struct Lock(File);

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.0);
    }
}

/// Acquire the per-workspace lock that prevents overlapping runs.
pub fn acquire_lock(ws: &Workspace) -> Result<Lock> {
    let path = store_root(ws).join("run.lock");
    fs::create_dir_all(path.parent().unwrap())?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)?;
    match FileExt::try_lock(&file) {
        Ok(()) => Ok(Lock(file)),
        Err(TryLockError::WouldBlock) => {
            Err(anyhow!("workspace lock is held at {}", path.display()))
        }
        Err(TryLockError::Error(err)) => {
            Err(err).with_context(|| format!("lock {}", path.display()))
        }
    }
}

/// Append one complete Source Snapshot to the source Item Store.
pub fn append_items(ws: &Workspace, source: &WorkspaceSource, items: &[Item]) -> Result<()> {
    append_items_with_cancellation(ws, source, items, &CancellationToken::new())
}

/// Publish item observations and their completion boundary as one Store update.
pub(crate) fn append_items_with_cancellation(
    ws: &Workspace,
    source: &WorkspaceSource,
    items: &[Item],
    cancellation: &CancellationToken,
) -> Result<()> {
    append_items_inner(ws, source, items, cancellation, |_| {})
}

#[derive(Clone, Copy)]
enum AppendPoint {
    ObservationAppended,
    BeforeBoundaryPublication,
}

#[cfg(test)]
fn append_items_with_hook(
    ws: &Workspace,
    source: &WorkspaceSource,
    items: &[Item],
    cancellation: &CancellationToken,
    hook: impl FnMut(AppendPoint),
) -> Result<()> {
    append_items_inner(ws, source, items, cancellation, hook)
}

fn append_items_inner(
    ws: &Workspace,
    source: &WorkspaceSource,
    items: &[Item],
    cancellation: &CancellationToken,
    mut hook: impl FnMut(AppendPoint),
) -> Result<()> {
    let path = items_path(ws, source);
    let snapshots = snapshot_path(&path);
    let snapshot_key = source_snapshot_key(source);
    let snapshot_id = next_snapshot_id();
    fs::create_dir_all(path.parent().unwrap())?;
    let temp = path.with_extension("jsonl.tmp");
    let snapshots_temp = snapshots.with_extension("snapshots.tmp");
    let result = (|| -> Result<()> {
        check_store_cancellation(cancellation)?;
        let mut staged_items = File::create(&temp)?;
        if path.exists() {
            let mut previous = File::open(&path)?;
            std::io::copy(&mut previous, &mut staged_items)?;
        }
        for item in items {
            check_store_cancellation(cancellation)?;
            let mut value = serde_json::to_value(item)?;
            let record = value
                .as_object_mut()
                .ok_or_else(|| anyhow!("serialized Item is not an object"))?;
            record.insert(
                SNAPSHOT_KEY_FIELD.into(),
                serde_json::Value::String(snapshot_key.clone()),
            );
            record.insert(
                SNAPSHOT_ID_FIELD.into(),
                serde_json::Value::String(snapshot_id.clone()),
            );
            writeln!(staged_items, "{value}")?;
            hook(AppendPoint::ObservationAppended);
            check_store_cancellation(cancellation)?;
        }
        staged_items.sync_all()?;

        check_store_cancellation(cancellation)?;
        let mut staged_boundary = File::create(&snapshots_temp)?;
        if snapshots.exists() {
            let mut previous = File::open(&snapshots)?;
            std::io::copy(&mut previous, &mut staged_boundary)?;
        }
        writeln!(
            staged_boundary,
            "{}",
            serde_json::json!({"snapshot_key": snapshot_key, "snapshot_id": snapshot_id})
        )?;
        staged_boundary.sync_all()?;
        check_store_cancellation(cancellation)?;

        // Publish the complete item file before the boundary. Partial observations stay historical.
        replace_file(&temp, &path)?;

        hook(AppendPoint::BeforeBoundaryPublication);
        check_store_cancellation(cancellation)?;

        // The boundary is the commit marker.
        replace_file(&snapshots_temp, &snapshots)
    })();
    let _ = fs::remove_file(&temp);
    let _ = fs::remove_file(&snapshots_temp);
    result
}

fn replace_file(temp: &Path, destination: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;

        #[link(name = "kernel32")]
        extern "system" {
            fn MoveFileExW(
                existing_file_name: *const u16,
                new_file_name: *const u16,
                flags: u32,
            ) -> i32;
        }

        let temp: Vec<u16> = temp
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let destination: Vec<u16> = destination
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        if unsafe {
            MoveFileExW(
                temp.as_ptr(),
                destination.as_ptr(),
                0x0000_0001 | 0x0000_0008,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error().into());
        }
        return Ok(());
    }

    #[cfg(not(windows))]
    {
        fs::rename(temp, destination)?;
        Ok(())
    }
}

fn check_store_cancellation(cancellation: &CancellationToken) -> Result<()> {
    if cancellation.is_cancelled() {
        Err(crate::runtime::InvocationCancelled.into())
    } else {
        Ok(())
    }
}

fn snapshot_path(path: &Path) -> PathBuf {
    path.with_extension("snapshots")
}

fn source_snapshot_key(source: &WorkspaceSource) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    source.configured.id.hash(&mut hasher);
    source.built.registration_id().hash(&mut hasher);
    source.built.config_json().hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn next_snapshot_id() -> String {
    format!(
        "{}-{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default(),
        SNAPSHOT_COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

/// Append one action attempt to the source action JSONL store.
pub fn append_action(
    ws: &Workspace,
    source: &WorkspaceSource,
    attempt: &ActionAttempt,
) -> Result<()> {
    let mut f = append_file(actions_path(ws, source))?;
    writeln!(f, "{}", serde_json::to_string(attempt)?)?;
    Ok(())
}

fn append_file(path: PathBuf) -> Result<File> {
    fs::create_dir_all(path.parent().unwrap())?;
    Ok(OpenOptions::new().create(true).append(true).open(path)?)
}

/// Return the latest observed item per item id across configured sources.
pub fn latest_items(ws: &Workspace) -> Result<HashMap<String, Item>> {
    Ok(latest_item_records(ws)?
        .into_iter()
        .map(|(key, stored)| (key, stored.item))
        .collect())
}

fn latest_item_records(ws: &Workspace) -> Result<HashMap<String, StoredItem>> {
    let mut map = HashMap::new();
    let mut records_by_path = HashMap::<PathBuf, Vec<StoredItem>>::new();
    let mut boundaries_by_path = HashMap::<PathBuf, HashMap<String, String>>::new();

    for source in &ws.sources {
        let path = items_path(ws, source);
        if !path.exists() {
            continue;
        }
        if !records_by_path.contains_key(&path) {
            // Read the boundary first. Publication replaces Items before the boundary, so this
            // order prevents a reader from pairing old Items with a new Snapshot ID.
            boundaries_by_path.insert(path.clone(), load_snapshot_boundaries(&path)?);
            records_by_path.insert(path.clone(), load_item_records(ws, &path)?);
        }

        let slug = source_slug(source);
        let snapshot_key = source_snapshot_key(source);
        let Some(snapshot_id) = boundaries_by_path
            .get(&path)
            .and_then(|boundaries| boundaries.get(&snapshot_key))
        else {
            continue;
        };
        for stored in records_by_path.get(&path).into_iter().flatten() {
            if stored.snapshot_key.as_deref() == Some(snapshot_key.as_str())
                && stored.snapshot_id.as_deref() == Some(snapshot_id.as_str())
            {
                map.insert(
                    item_key(&slug, &stored.item.id),
                    StoredItem {
                        slug: slug.clone(),
                        item: stored.item.clone(),
                        snapshot_key: stored.snapshot_key.clone(),
                        snapshot_id: stored.snapshot_id.clone(),
                    },
                );
            }
        }
    }
    Ok(map)
}

fn load_item_records(ws: &Workspace, path: &Path) -> Result<Vec<StoredItem>> {
    let file = File::open(path).with_context(|| format!("open item Store {}", path.display()))?;
    let mut records = Vec::new();
    for (index, line) in BufReader::new(file).lines().enumerate() {
        let line_number = index + 1;
        let line =
            line.with_context(|| format!("read item Store {} line {line_number}", path.display()))?;
        let value: Value = serde_json::from_str(&line)
            .with_context(|| format!("parse item Store {} line {line_number}", path.display()))?;
        let Some(record) = value.as_object() else {
            bail!(
                "load item Store {} line {line_number}; Item record must be an object",
                path.display()
            );
        };
        if !record.contains_key("reference_id") {
            bail!(
                "load item Store {} line {line_number}; item records now require reference_id. Remove {} and run `agentboard run {}` to rebuild the affected Store",
                path.display(),
                path.display(),
                ws.path.display()
            );
        }
        let snapshot_key = record
            .get(SNAPSHOT_KEY_FIELD)
            .and_then(Value::as_str)
            .map(str::to_owned);
        let snapshot_id = record
            .get(SNAPSHOT_ID_FIELD)
            .and_then(Value::as_str)
            .map(str::to_owned);
        let item: Item = serde_json::from_value(value)
            .with_context(|| format!("load item Store {} line {line_number}", path.display()))?;
        records.push(StoredItem {
            slug: String::new(),
            item,
            snapshot_key,
            snapshot_id,
        });
    }
    Ok(records)
}

fn load_snapshot_boundaries(path: &Path) -> Result<HashMap<String, String>> {
    let snapshots = snapshot_path(path);
    if !snapshots.exists() {
        return Ok(HashMap::new());
    }
    let file = File::open(&snapshots).with_context(|| {
        format!(
            "open item Store snapshot boundaries {}",
            snapshots.display()
        )
    })?;
    let mut latest = HashMap::new();
    for (index, line) in BufReader::new(file).lines().enumerate() {
        let line_number = index + 1;
        let value: Value = serde_json::from_str(&line?).with_context(|| {
            format!(
                "parse item Store snapshot boundary {} line {line_number}",
                snapshots.display()
            )
        })?;
        let key = value
            .get("snapshot_key")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("snapshot boundary missing snapshot_key"))?;
        let id = value
            .get("snapshot_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("snapshot boundary missing snapshot_id"))?;
        latest.insert(key.to_string(), id.to_string());
    }
    Ok(latest)
}

/// Return every stored action attempt for the workspace.
pub fn all_actions(ws: &Workspace) -> Result<Vec<ActionAttempt>> {
    Ok(all_stored_actions(ws)?
        .into_iter()
        .map(|stored| stored.attempt)
        .collect())
}

fn all_stored_actions(ws: &Workspace) -> Result<Vec<StoredAction>> {
    let mut out = Vec::new();
    let mut seen_paths = HashSet::new();
    for source in &ws.sources {
        let path = actions_path(ws, source);
        if !seen_paths.insert(path.clone()) || !path.exists() {
            continue;
        }
        let slug = source_slug(source);
        for line in BufReader::new(File::open(path)?).lines() {
            out.push(StoredAction {
                slug: slug.clone(),
                attempt: serde_json::from_str(&line?)?,
            });
        }
    }
    Ok(out)
}

/// Return identity keys for successful actions, used to skip already-completed work.
pub fn successful_actions(ws: &Workspace, source: &WorkspaceSource) -> Result<HashSet<String>> {
    let path = actions_path(ws, source);
    if !path.exists() {
        return Ok(HashSet::new());
    }
    let mut latest = HashMap::new();
    for line in BufReader::new(File::open(path)?).lines() {
        let a: ActionAttempt = serde_json::from_str(&line?)?;
        latest.insert(
            action_key(
                &a.source_id,
                &a.item_id,
                a.source_action_index,
                &a.rendered_action_hash,
            ),
            a.outcome,
        );
    }
    Ok(latest
        .into_iter()
        .filter_map(|(key, outcome)| (outcome == ActionOutcome::Success).then_some(key))
        .collect())
}

/// Print latest stored items with derived action state.
pub fn list_items(ws: &Workspace, as_json: bool) -> Result<()> {
    let mut items: Vec<_> = latest_item_records(ws)?.into_values().collect();
    items.sort_by(|a, b| a.item.id.cmp(&b.item.id));
    let actions = all_stored_actions(ws)?;
    if as_json {
        let rows: Vec<_> = items
            .into_iter()
            .map(|item| json!({ "action_state": action_state(&actions, &item), "source_slug": item.slug, "item": item.item }))
            .collect();
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        for item in items {
            println!(
                "{}\t{}\t{}\t{}",
                item.item.id,
                item.item.status,
                action_state(&actions, &item),
                item.item.title
            );
        }
    }
    Ok(())
}

// Derive display state from action attempts for one item.
fn action_state(actions: &[StoredAction], item: &StoredItem) -> &'static str {
    let mut latest = HashMap::new();
    for action in actions.iter().filter(|a| action_matches_item(a, item)) {
        latest.insert(action.attempt.source_action_index, action.attempt.outcome);
    }
    if latest
        .values()
        .any(|outcome| *outcome == ActionOutcome::Failure)
    {
        "failed"
    } else if latest
        .values()
        .any(|outcome| *outcome == ActionOutcome::Cancelled)
    {
        "pending"
    } else if !latest.is_empty() {
        "succeeded"
    } else {
        "pending"
    }
}

/// Print one latest stored item and its action attempts.
pub fn show_item(ws: &Workspace, item_ref: &str, as_json: bool) -> Result<()> {
    let item = resolve_item(ws, item_ref)?;
    let actions: Vec<_> = all_stored_actions(ws)?
        .into_iter()
        .filter(|action| action_matches_item(action, &item))
        .map(|stored| stored.attempt)
        .collect();
    if as_json {
        println!(
            "{}",
            serde_json::to_string_pretty(
                &json!({"source_slug": item.slug, "item": item.item, "actions": actions})
            )?
        );
    } else {
        println!(
            "{}\n{}\n{}\n{}",
            item.item.id, item.item.title, item.item.status, item.item.url
        );
        for a in actions {
            println!(
                "action#{} {} outcome={}",
                a.source_action_index, a.uses, a.outcome
            );
        }
    }
    Ok(())
}

fn resolve_item(ws: &Workspace, item_ref: &str) -> Result<StoredItem> {
    let mut records = latest_item_records(ws)?;
    if let Some((slug, id)) = item_ref.split_once(':') {
        return records
            .remove(&item_key(slug, id))
            .ok_or_else(|| anyhow!("item {item_ref} not found"));
    }

    let matches: Vec<_> = records
        .into_values()
        .filter(|stored| stored.item.id == item_ref)
        .collect();
    match matches.as_slice() {
        [] => Err(anyhow!("item {item_ref} not found")),
        [item] => Ok(item.clone()),
        _ => {
            let mut refs: Vec<_> = matches
                .iter()
                .map(|stored| format!("{}:{}", stored.slug, stored.item.id))
                .collect();
            refs.sort();
            Err(anyhow!(
                "item {item_ref} is ambiguous across Store item buckets; use one of: {}",
                refs.join(", ")
            ))
        }
    }
}

/// Validate the Store, registered Source reachability, and registered Action health.
pub async fn doctor(
    ws: &Workspace,
    registry: &Registry,
    output: &Output,
    cancellation: CancellationToken,
) -> Result<()> {
    output.info(
        "doctor.start",
        &format!("doctor {} starting", ws.id),
        json!({"workspace": ws.id}),
    )?;
    let mut failures = 0_usize;

    if cancellation.is_cancelled() {
        output.info(
            "doctor.cancelled",
            &format!("doctor {} cancellation observed between work units", ws.id),
            json!({"workspace": ws.id, "outcome": "cancelled"}),
        )?;
        return Err(crate::runtime::InvocationCancelled.into());
    }

    // Registry loading already completed strict config validation before diagnostics.
    report_check(output, ws, "config", Ok(()), &mut failures)?;

    let root = store_root(ws);
    let probe = root.join(".doctor-write-test");
    let store = (|| -> Result<()> {
        fs::create_dir_all(&root)?;
        fs::write(&probe, b"ok")?;
        fs::remove_file(&probe)?;
        Ok(())
    })();
    let _ = fs::remove_file(&probe);
    report_check(output, ws, "store", store, &mut failures)?;

    for source in &ws.sources {
        if cancellation.is_cancelled() {
            output.info(
                "doctor.cancelled",
                &format!("doctor {} cancellation observed between work units", ws.id),
                json!({"workspace": ws.id, "outcome": "cancelled"}),
            )?;
            return Err(crate::runtime::InvocationCancelled.into());
        }
        let source_id = &source.configured.id;
        let context = HealthCheckContext {
            source_id,
            cancellation: cancellation.clone(),
        };
        let health = source.built.runtime().health_check(&context).await;
        if cancellation.is_cancelled() {
            output.info(
                "doctor.cancelled",
                &format!("doctor {} cancellation observed between work units", ws.id),
                json!({"workspace": ws.id, "outcome": "cancelled"}),
            )?;
            return Err(crate::runtime::InvocationCancelled.into());
        }
        match health {
            Ok(collection) => output.success(
                "doctor.check",
                &source_reachable_message(source_id, &collection),
                json!({
                    "workspace": ws.id,
                    "check": "source",
                    "source": source_id,
                    "outcome": "pass",
                    "fetched": collection.items.len(),
                    "available": collection.available,
                    "limit": collection.limit,
                }),
            )?,
            Err(err) => {
                failures += 1;
                output.error(
                    "doctor.check",
                    &format!("fail source {source_id}: {err:#}"),
                    json!({"workspace": ws.id, "check": "source", "source": source_id, "outcome": "fail", "error": format!("{err:#}")}),
                )?;
            }
        }
        if cancellation.is_cancelled() {
            output.info(
                "doctor.cancelled",
                &format!("doctor {} cancellation observed between work units", ws.id),
                json!({"workspace": ws.id, "outcome": "cancelled"}),
            )?;
            return Err(crate::runtime::InvocationCancelled.into());
        }
        output.info(
            "doctor.actions",
            &format!("actions [{}]", source.configured.actions.len()),
            json!({"workspace": ws.id, "source": source_id, "actions": source.configured.actions.len()}),
        )?;
        for (index, action) in source.configured.actions.iter().enumerate() {
            if cancellation.is_cancelled() {
                output.info(
                    "doctor.cancelled",
                    &format!("doctor {} cancellation observed between work units", ws.id),
                    json!({"workspace": ws.id, "outcome": "cancelled"}),
                )?;
                return Err(crate::runtime::InvocationCancelled.into());
            }
            match registry.check_action(&action.uses, &context) {
                Ok(()) => output.success(
                    "doctor.action",
                    &format!("  - {} [ok]", action.uses),
                    json!({"workspace": ws.id, "source": source_id, "action_index": index, "uses": action.uses, "outcome": "pass"}),
                )?,
                Err(err) => {
                    failures += 1;
                    let detail = std::error::Error::source(&err)
                        .map(ToString::to_string)
                        .unwrap_or_else(|| err.to_string());
                    output.error(
                        "doctor.action",
                        &format!("  - {} [fail: {detail}]", action.uses),
                        json!({"workspace": ws.id, "source": source_id, "action_index": index, "uses": action.uses, "outcome": "fail", "error": detail}),
                    )?;
                }
            }
        }
        if cancellation.is_cancelled() {
            output.info(
                "doctor.cancelled",
                &format!("doctor {} cancellation observed between work units", ws.id),
                json!({"workspace": ws.id, "outcome": "cancelled"}),
            )?;
            return Err(crate::runtime::InvocationCancelled.into());
        }
        for check in source.built.runtime().health_checks(&context) {
            report_check(output, ws, &check.name, check.result, &mut failures)?;
            if cancellation.is_cancelled() {
                output.info(
                    "doctor.cancelled",
                    &format!("doctor {} cancellation observed between work units", ws.id),
                    json!({"workspace": ws.id, "outcome": "cancelled"}),
                )?;
                return Err(crate::runtime::InvocationCancelled.into());
            }
        }
    }

    if cancellation.is_cancelled() {
        output.info(
            "doctor.cancelled",
            &format!("doctor {} cancellation observed between work units", ws.id),
            json!({"workspace": ws.id, "outcome": "cancelled"}),
        )?;
        return Err(crate::runtime::InvocationCancelled.into());
    }

    if failures > 0 {
        output.error(
            "doctor.failed",
            &format!("doctor {} failed: {failures} check(s)", ws.id),
            json!({"workspace": ws.id, "failed": failures}),
        )?;
        bail!("doctor found {failures} failed check(s)");
    }
    output.success(
        "doctor.complete",
        &format!("doctor {} complete: all checks passed", ws.id),
        json!({"workspace": ws.id, "failed": 0}),
    )?;
    Ok(())
}

fn source_reachable_message(source_id: &str, collection: &SourceCollection) -> String {
    match collection.available {
        Some(available) => format!(
            "ok source {source_id} reachable ({available} available; {} fetched; limit {})",
            collection.items.len(),
            collection.limit
        ),
        None => format!(
            "ok source {source_id} reachable ({} fetched; limit {}; available unknown)",
            collection.items.len(),
            collection.limit
        ),
    }
}

fn report_check(
    output: &Output,
    ws: &Workspace,
    check: &str,
    result: Result<()>,
    failures: &mut usize,
) -> Result<()> {
    match result {
        Ok(()) => output.success(
            "doctor.check",
            &format!("ok {check}"),
            json!({"workspace": ws.id, "check": check, "outcome": "pass"}),
        ),
        Err(err) => {
            *failures += 1;
            output.error(
                "doctor.check",
                &format!("fail {check}: {err:#}"),
                json!({"workspace": ws.id, "check": check, "outcome": "fail", "error": format!("{err:#}")}),
            )
        }
    }
}

fn action_matches_item(action: &StoredAction, item: &StoredItem) -> bool {
    action.slug == item.slug && action.attempt.item_id == item.item.id
}

fn item_key(source_slug: &str, item_id: &str) -> String {
    format!("{source_slug}\0{item_id}")
}

/// Build the stable identity key for one rendered source action.
pub fn action_key(source_id: &str, item_id: &str, idx: usize, hash: &str) -> String {
    format!("{source_id}\0{item_id}\0{idx}\0{hash}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::parse_workspace;
    use serde_json::json;

    static TEST_WORKSPACE_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn doctor_source_message_distinguishes_available_from_fetched() {
        let known = SourceCollection {
            items: vec![],
            available: Some(237),
            limit: 50,
        };
        assert_eq!(
            source_reachable_message("github", &known),
            "ok source github reachable (237 available; 0 fetched; limit 50)"
        );

        let unknown = SourceCollection {
            items: vec![],
            available: None,
            limit: 50,
        };
        assert_eq!(
            source_reachable_message("qmd", &unknown),
            "ok source qmd reachable (0 fetched; limit 50; available unknown)"
        );
    }

    #[test]
    fn same_jira_site_shares_latest_item_observations() {
        let ws = workspace(&[
            ("open", "https://team-a.atlassian.net", "project = AB"),
            (
                "mine",
                "https://team-a.atlassian.net/",
                "assignee = currentUser()",
            ),
        ]);
        let _cleanup = StoreCleanup::new(&ws);

        append_items(&ws, &ws.sources[0], &[item("open", "PROJ-1")]).unwrap();
        append_items(&ws, &ws.sources[1], &[item("mine", "PROJ-1")]).unwrap();

        let items: Vec<_> = latest_items(&ws).unwrap().into_values().collect();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "mine");
    }

    #[test]
    fn different_jira_sites_do_not_collide_on_issue_key() {
        let ws = workspace(&[
            ("a", "https://team-a.atlassian.net", "project = AB"),
            ("b", "https://team-b.atlassian.net", "project = AB"),
        ]);
        let _cleanup = StoreCleanup::new(&ws);

        append_items(&ws, &ws.sources[0], &[item("from a", "PROJ-1")]).unwrap();
        append_items(&ws, &ws.sources[1], &[item("from b", "PROJ-1")]).unwrap();

        let items = latest_items(&ws).unwrap();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn legacy_item_store_error_explains_how_to_rebuild() {
        let ws = workspace(&[("jira", "https://team-a.atlassian.net", "project = AB")]);
        let _cleanup = StoreCleanup::new(&ws);
        let path = items_path(&ws, &ws.sources[0]);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            r#"{"id":"10001","title":"Legacy","status":"open","url":"https://example.test","source_id":"jira","source_kind":"jira","raw":{}}
"#,
        )
        .unwrap();

        let error = latest_items(&ws).unwrap_err();
        let message = format!("{error:#}");

        assert!(message.contains(&path.display().to_string()));
        assert!(message.contains("line 1"));
        assert!(message.contains("reference_id"));
        assert!(message.contains("rebuild"));
    }

    #[test]
    fn malformed_item_store_keeps_parse_error_without_rebuild_advice() {
        let ws = workspace(&[("jira", "https://team-a.atlassian.net", "project = AB")]);
        let _cleanup = StoreCleanup::new(&ws);
        let path = items_path(&ws, &ws.sources[0]);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "not json\n").unwrap();

        let error = latest_items(&ws).unwrap_err();
        let message = format!("{error:#}");

        assert!(message.contains(&path.display().to_string()));
        assert!(message.contains("line 1"));
        assert!(!message.contains("reference_id"));
        assert!(!message.contains("rebuild"));
    }

    #[test]
    fn successful_actions_are_scoped_to_source_hash() {
        let old = workspace(&[("jira", "https://team-a.atlassian.net", "project = AB")]);
        let ws = workspace(&[(
            "jira",
            "https://team-a.atlassian.net",
            "assignee = currentUser()",
        )]);
        let _cleanup = StoreCleanup::new(&ws);

        append_action(&ws, &old.sources[0], &attempt("jira", "PROJ-1", true)).unwrap();

        assert!(successful_actions(&ws, &ws.sources[0]).unwrap().is_empty());
    }

    #[test]
    fn action_state_matches_item_bucket_not_source_label() {
        let ws = workspace(&[
            ("a", "https://team-a.atlassian.net", "project = AB"),
            (
                "b",
                "https://team-a.atlassian.net",
                "assignee = currentUser()",
            ),
        ]);
        let _cleanup = StoreCleanup::new(&ws);
        append_items(
            &ws,
            &ws.sources[1],
            &[Item {
                source_id: "b".into(),
                ..item("from b", "PROJ-1")
            }],
        )
        .unwrap();
        append_action(&ws, &ws.sources[0], &attempt("a", "PROJ-1", true)).unwrap();

        let item = resolve_item(&ws, "PROJ-1").unwrap();
        let actions = all_stored_actions(&ws).unwrap();

        assert_eq!(action_state(&actions, &item), "succeeded");
    }

    #[test]
    fn cancelled_latest_action_is_pending_and_retryable() {
        let ws = workspace(&[("a", "https://team-a.atlassian.net", "project = AB")]);
        let _cleanup = StoreCleanup::new(&ws);
        append_items(&ws, &ws.sources[0], &[item("from a", "PROJ-1")]).unwrap();
        append_action(&ws, &ws.sources[0], &attempt("a", "PROJ-1", true)).unwrap();
        let mut cancelled = attempt("a", "PROJ-1", true);
        cancelled.outcome = ActionOutcome::Cancelled;
        append_action(&ws, &ws.sources[0], &cancelled).unwrap();

        let stored = all_stored_actions(&ws).unwrap();
        let item = resolve_item(&ws, "PROJ-1").unwrap();

        assert_eq!(action_state(&stored, &item), "pending");
        assert!(successful_actions(&ws, &ws.sources[0]).unwrap().is_empty());
    }

    #[test]
    fn cancelled_latest_attempt_overrides_an_older_failure_with_another_hash() {
        let ws = workspace(&[("a", "https://team-a.atlassian.net", "project = AB")]);
        let _cleanup = StoreCleanup::new(&ws);
        append_items(&ws, &ws.sources[0], &[item("from a", "PROJ-1")]).unwrap();

        let mut failed = attempt("a", "PROJ-1", false);
        failed.rendered_action_hash = "hash-a".into();
        append_action(&ws, &ws.sources[0], &failed).unwrap();

        let mut cancelled = attempt("a", "PROJ-1", true);
        cancelled.rendered_action_hash = "hash-b".into();
        cancelled.outcome = ActionOutcome::Cancelled;
        append_action(&ws, &ws.sources[0], &cancelled).unwrap();

        let stored = all_stored_actions(&ws).unwrap();
        let item = resolve_item(&ws, "PROJ-1").unwrap();

        assert_eq!(action_state(&stored, &item), "pending");
    }

    #[test]
    fn latest_complete_snapshot_defines_current_membership() {
        let ws = workspace(&[("a", "https://team-a.atlassian.net", "project = AB")]);
        let _cleanup = StoreCleanup::new(&ws);
        append_items(&ws, &ws.sources[0], &[item("old", "PROJ-1")]).unwrap();
        assert_eq!(latest_items(&ws).unwrap().len(), 1);

        append_items(&ws, &ws.sources[0], &[]).unwrap();

        assert!(latest_items(&ws).unwrap().is_empty());
        assert!(snapshot_path(&items_path(&ws, &ws.sources[0])).exists());
    }

    #[test]
    fn cancelled_snapshot_does_not_replace_previous_snapshot() {
        let ws = workspace(&[("a", "https://team-a.atlassian.net", "project = AB")]);
        let _cleanup = StoreCleanup::new(&ws);
        append_items(&ws, &ws.sources[0], &[item("old", "PROJ-1")]).unwrap();

        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let error = append_items_with_cancellation(
            &ws,
            &ws.sources[0],
            &[item("new", "PROJ-1")],
            &cancellation,
        )
        .unwrap_err();

        assert!(error
            .downcast_ref::<crate::runtime::InvocationCancelled>()
            .is_some());
        let stored = latest_items(&ws).unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored.values().next().unwrap().title, "old");
        assert_temp_files_are_clean(&ws, &ws.sources[0]);

        append_items(&ws, &ws.sources[0], &[item("new", "PROJ-1")]).unwrap();
        assert_eq!(
            latest_items(&ws).unwrap().values().next().unwrap().title,
            "new"
        );
    }

    #[test]
    fn cancelled_during_observation_append_keeps_previous_snapshot() {
        let ws = workspace(&[("a", "https://team-a.atlassian.net", "project = AB")]);
        let _cleanup = StoreCleanup::new(&ws);
        append_items(&ws, &ws.sources[0], &[item("old", "PROJ-1")]).unwrap();

        let cancellation = CancellationToken::new();
        let error = append_items_with_hook(
            &ws,
            &ws.sources[0],
            &[item("new", "PROJ-1"), item("newer", "PROJ-2")],
            &cancellation,
            |point| {
                if matches!(point, AppendPoint::ObservationAppended) {
                    cancellation.cancel();
                }
            },
        )
        .unwrap_err();

        assert!(is_store_cancelled(&error));
        assert_eq!(
            latest_items(&ws).unwrap().values().next().unwrap().title,
            "old"
        );
        assert_eq!(snapshot_boundary_count(&ws, &ws.sources[0]), 1);
        assert_temp_files_are_clean(&ws, &ws.sources[0]);
    }

    #[test]
    fn cancelled_before_boundary_publication_keeps_previous_snapshot() {
        let ws = workspace(&[("a", "https://team-a.atlassian.net", "project = AB")]);
        let _cleanup = StoreCleanup::new(&ws);
        append_items(&ws, &ws.sources[0], &[item("old", "PROJ-1")]).unwrap();

        let cancellation = CancellationToken::new();
        let error = append_items_with_hook(
            &ws,
            &ws.sources[0],
            &[item("new", "PROJ-1")],
            &cancellation,
            |point| {
                if matches!(point, AppendPoint::BeforeBoundaryPublication) {
                    cancellation.cancel();
                }
            },
        )
        .unwrap_err();

        assert!(is_store_cancelled(&error));
        assert_eq!(
            latest_items(&ws).unwrap().values().next().unwrap().title,
            "old"
        );
        assert_eq!(snapshot_boundary_count(&ws, &ws.sources[0]), 1);
        assert_temp_files_are_clean(&ws, &ws.sources[0]);
    }

    #[test]
    fn completed_snapshot_remains_authoritative_after_later_cancellation() {
        let ws = workspace(&[("a", "https://team-a.atlassian.net", "project = AB")]);
        let _cleanup = StoreCleanup::new(&ws);
        let cancellation = CancellationToken::new();

        append_items_with_cancellation(
            &ws,
            &ws.sources[0],
            &[item("new", "PROJ-1")],
            &cancellation,
        )
        .unwrap();
        cancellation.cancel();

        assert_eq!(
            latest_items(&ws).unwrap().values().next().unwrap().title,
            "new"
        );
        assert_eq!(snapshot_boundary_count(&ws, &ws.sources[0]), 1);
    }

    #[test]
    fn append_failure_cleans_staged_files() {
        let ws = workspace(&[("a", "https://team-a.atlassian.net", "project = AB")]);
        let _cleanup = StoreCleanup::new(&ws);
        let path = items_path(&ws, &ws.sources[0]);
        let snapshots_temp = snapshot_path(&path).with_extension("snapshots.tmp");
        fs::create_dir_all(&snapshots_temp).unwrap();

        assert!(append_items(&ws, &ws.sources[0], &[item("new", "PROJ-1")]).is_err());
        assert!(!path.with_extension("jsonl.tmp").exists());
    }

    #[test]
    fn qualified_item_ref_disambiguates_item_bucket() {
        let ws = workspace(&[
            ("a", "https://team-a.atlassian.net", "project = AB"),
            ("b", "https://team-b.atlassian.net", "project = AB"),
        ]);
        let _cleanup = StoreCleanup::new(&ws);
        append_items(&ws, &ws.sources[0], &[item("from a", "PROJ-1")]).unwrap();
        append_items(&ws, &ws.sources[1], &[item("from b", "PROJ-1")]).unwrap();

        let slug = source_slug(&ws.sources[1]);
        assert_eq!(
            resolve_item(&ws, &format!("{slug}:PROJ-1"))
                .unwrap()
                .item
                .title,
            "from b"
        );
        assert!(resolve_item(&ws, "PROJ-1")
            .unwrap_err()
            .to_string()
            .contains("ambiguous"));
    }

    fn is_store_cancelled(error: &anyhow::Error) -> bool {
        error
            .downcast_ref::<crate::runtime::InvocationCancelled>()
            .is_some()
    }

    fn snapshot_boundary_count(ws: &Workspace, source: &WorkspaceSource) -> usize {
        fs::read_to_string(snapshot_path(&items_path(ws, source)))
            .unwrap()
            .lines()
            .count()
    }

    fn assert_temp_files_are_clean(ws: &Workspace, source: &WorkspaceSource) {
        let path = items_path(ws, source);
        assert!(!path.with_extension("jsonl.tmp").exists());
        assert!(!snapshot_path(&path)
            .with_extension("snapshots.tmp")
            .exists());
    }

    fn workspace(sources: &[(&str, &str, &str)]) -> Workspace {
        let text = sources
            .iter()
            .map(|(id, site, jql)| {
                format!(
                    r#"
[[sources]]
id = {id:?}
[sources.source]
kind = "jira"
site = {site:?}
jql = {jql:?}
"#
                )
            })
            .collect::<String>();
        let registry = crate::cli::register_builtins().unwrap();
        let parsed = parse_workspace(&text, &registry).unwrap();
        Workspace {
            id: format!(
                "test-{}-{}",
                std::process::id(),
                TEST_WORKSPACE_COUNTER.fetch_add(1, Ordering::Relaxed)
            ),
            path: "work.toml".into(),
            sources: parsed.sources,
        }
    }

    fn item(title: &str, id: &str) -> Item {
        Item {
            id: id.into(),
            reference_id: id.into(),
            title: title.into(),
            status: "open".into(),
            url: "https://example.test".into(),
            source_id: "jira".into(),
            source_kind: "jira".into(),
            raw: json!({}),
        }
    }

    fn attempt(source_id: &str, item_id: &str, success: bool) -> ActionAttempt {
        ActionAttempt {
            ts: "2026-01-01T00:00:00Z".into(),
            source_id: source_id.into(),
            item_id: item_id.into(),
            source_action_index: 0,
            uses: "agentboard/run-cmd".into(),
            rendered_action_hash: "abc123".into(),
            outcome: if success {
                ActionOutcome::Success
            } else {
                ActionOutcome::Failure
            },
            stdout: String::new(),
            stderr: String::new(),
            message: None,
        }
    }

    struct StoreCleanup(PathBuf);

    impl StoreCleanup {
        fn new(ws: &Workspace) -> Self {
            Self(store_root(ws))
        }
    }

    impl Drop for StoreCleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Command as ProcessCommand, Stdio},
};

use agentboard_core::model::{ActionAttempt, Item, SourceConfig, SourceKind, Workspace};
use anyhow::{anyhow, bail, Context, Result};
use fs4::{FileExt, TryLockError};
use serde_json::json;

use crate::{
    adapters::{inspect_source, SourceInspection},
    config::{actions_path, items_path, source_slug, store_root},
    output::Output,
};

#[derive(Clone, Debug)]
struct StoredItem {
    slug: String,
    item: Item,
}

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

/// Append item observations for one source to its JSONL store.
pub fn append_items(ws: &Workspace, source: &SourceConfig, items: &[Item]) -> Result<()> {
    let mut f = append_file(items_path(ws, source))?;
    for item in items {
        writeln!(f, "{}", serde_json::to_string(item)?)?;
    }
    Ok(())
}

/// Append one action attempt to the source action JSONL store.
pub fn append_action(ws: &Workspace, source: &SourceConfig, attempt: &ActionAttempt) -> Result<()> {
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
    let mut seen_paths = HashSet::new();
    for source in &ws.config.sources {
        let path = items_path(ws, source);
        if !seen_paths.insert(path.clone()) || !path.exists() {
            continue;
        }
        let slug = source_slug(source);
        for line in BufReader::new(File::open(path)?).lines() {
            let item: Item = serde_json::from_str(&line?)?;
            map.insert(
                item_key(&slug, &item.id),
                StoredItem {
                    slug: slug.clone(),
                    item,
                },
            );
        }
    }
    Ok(map)
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
    for source in &ws.config.sources {
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
pub fn successful_actions(ws: &Workspace, source: &SourceConfig) -> Result<HashSet<String>> {
    let path = actions_path(ws, source);
    if !path.exists() {
        return Ok(HashSet::new());
    }
    let mut out = HashSet::new();
    for line in BufReader::new(File::open(path)?).lines() {
        let a: ActionAttempt = serde_json::from_str(&line?)?;
        if a.success {
            out.insert(action_key(
                &a.source_id,
                &a.item_id,
                a.source_action_index,
                &a.rendered_action_hash,
            ));
        }
    }
    Ok(out)
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
    let mut saw_action = false;
    let mut saw_failure = false;
    for action in actions.iter().filter(|a| action_matches_item(a, item)) {
        saw_action = true;
        saw_failure |= !action.attempt.success;
    }
    if saw_failure {
        "failed"
    } else if saw_action {
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
                "action#{} {} success={}",
                a.source_action_index, a.uses, a.success
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

/// Validate config, Store writability, Source reachability, and required commands.
pub async fn doctor(ws: &Workspace, output: &Output) -> Result<()> {
    output.info(
        "doctor.start",
        &format!("doctor {} starting", ws.id),
        json!({"workspace": ws.id}),
    )?;
    let mut failures = 0_usize;

    let config = crate::config::validate_config(&ws.config);
    report_check(output, ws, "config", config, &mut failures)?;

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

    let mut commands = HashSet::new();
    for source in &ws.config.sources {
        if matches!(&source.source, SourceKind::Qmd { .. }) {
            commands.insert("qmd");
        }
        match inspect_source(source).await {
            Ok(inspection) => output.success(
                "doctor.check",
                &source_reachable_message(&source.id, &inspection),
                json!({
                    "workspace": ws.id,
                    "check": "source",
                    "source": source.id,
                    "outcome": "pass",
                    "fetched": inspection.items.len(),
                    "available": inspection.available,
                    "limit": inspection.limit,
                }),
            )?,
            Err(err) => {
                failures += 1;
                output.error(
                    "doctor.check",
                    &format!("fail source {}: {err:#}", source.id),
                    json!({"workspace": ws.id, "check": "source", "source": source.id, "outcome": "fail", "error": format!("{err:#}")}),
                )?;
            }
        }
        output.info(
            "doctor.actions",
            &format!("actions [{}]", source.actions.len()),
            json!({"workspace": ws.id, "source": source.id, "actions": source.actions.len()}),
        )?;
        for (index, action) in source.actions.iter().enumerate() {
            match check_action(action) {
                Ok(()) => output.success(
                    "doctor.action",
                    &format!("  - {} [ok]", action.uses),
                    json!({"workspace": ws.id, "source": source.id, "action_index": index, "uses": action.uses, "outcome": "pass"}),
                )?,
                Err(err) => {
                    failures += 1;
                    output.error(
                        "doctor.action",
                        &format!("  - {} [fail: {err:#}]", action.uses),
                        json!({"workspace": ws.id, "source": source.id, "action_index": index, "uses": action.uses, "outcome": "fail", "error": format!("{err:#}")}),
                    )?;
                }
            }
        }
    }
    for command in commands {
        report_check(
            output,
            ws,
            &format!("command {command}"),
            command_exists(command),
            &mut failures,
        )?;
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

fn check_action(action: &agentboard_core::model::ActionConfig) -> Result<()> {
    crate::config::validate_action(action)?;
    match action.uses.as_str() {
        "agentboard/run-cmd" => command_exists("sh"),
        "agentboard/create-worktree" => command_exists("git"),
        _ => Ok(()),
    }
}

fn source_reachable_message(source_id: &str, inspection: &SourceInspection) -> String {
    match inspection.available {
        Some(available) => format!(
            "ok source {source_id} reachable ({available} available; {} fetched; limit {})",
            inspection.items.len(),
            inspection.limit
        ),
        None => format!(
            "ok source {source_id} reachable ({} fetched; limit {}; available unknown)",
            inspection.items.len(),
            inspection.limit
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

fn command_exists(cmd: &str) -> Result<()> {
    let status = ProcessCommand::new(cmd)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("required command {cmd} not found"))?;
    if !status.success() {
        bail!("required command {cmd} returned {status}");
    }
    Ok(())
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
    use agentboard_core::model::{SourceConfig, SourceKind, WorkspaceConfig};
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn doctor_source_message_distinguishes_available_from_fetched() {
        let known = SourceInspection {
            items: vec![],
            available: Some(237),
            limit: 50,
        };
        assert_eq!(
            source_reachable_message("github", &known),
            "ok source github reachable (237 available; 0 fetched; limit 50)"
        );

        let unknown = SourceInspection {
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
        let ws = workspace(vec![
            jira_source("open", "https://team-a.atlassian.net", "project = AB"),
            jira_source(
                "mine",
                "https://team-a.atlassian.net/",
                "assignee = currentUser()",
            ),
        ]);
        let _cleanup = StoreCleanup::new(&ws);

        append_items(&ws, &ws.config.sources[0], &[item("open", "PROJ-1")]).unwrap();
        append_items(&ws, &ws.config.sources[1], &[item("mine", "PROJ-1")]).unwrap();

        let items: Vec<_> = latest_items(&ws).unwrap().into_values().collect();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "mine");
    }

    #[test]
    fn different_jira_sites_do_not_collide_on_issue_key() {
        let ws = workspace(vec![
            jira_source("a", "https://team-a.atlassian.net", "project = AB"),
            jira_source("b", "https://team-b.atlassian.net", "project = AB"),
        ]);
        let _cleanup = StoreCleanup::new(&ws);

        append_items(&ws, &ws.config.sources[0], &[item("from a", "PROJ-1")]).unwrap();
        append_items(&ws, &ws.config.sources[1], &[item("from b", "PROJ-1")]).unwrap();

        let items = latest_items(&ws).unwrap();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn successful_actions_are_scoped_to_source_hash() {
        let old_source = jira_source("jira", "https://team-a.atlassian.net", "project = AB");
        let new_source = jira_source(
            "jira",
            "https://team-a.atlassian.net",
            "assignee = currentUser()",
        );
        let ws = workspace(vec![new_source.clone()]);
        let _cleanup = StoreCleanup::new(&ws);

        append_action(&ws, &old_source, &attempt("jira", "PROJ-1", true)).unwrap();

        assert!(successful_actions(&ws, &new_source).unwrap().is_empty());
    }

    #[test]
    fn action_state_matches_item_bucket_not_source_label() {
        let ws = workspace(vec![
            jira_source("a", "https://team-a.atlassian.net", "project = AB"),
            jira_source(
                "b",
                "https://team-a.atlassian.net",
                "assignee = currentUser()",
            ),
        ]);
        let _cleanup = StoreCleanup::new(&ws);
        append_items(
            &ws,
            &ws.config.sources[1],
            &[Item {
                source_id: "b".into(),
                ..item("from b", "PROJ-1")
            }],
        )
        .unwrap();
        append_action(&ws, &ws.config.sources[0], &attempt("a", "PROJ-1", true)).unwrap();

        let item = resolve_item(&ws, "PROJ-1").unwrap();
        let actions = all_stored_actions(&ws).unwrap();

        assert_eq!(action_state(&actions, &item), "succeeded");
    }

    #[test]
    fn qualified_item_ref_disambiguates_item_bucket() {
        let ws = workspace(vec![
            jira_source("a", "https://team-a.atlassian.net", "project = AB"),
            jira_source("b", "https://team-b.atlassian.net", "project = AB"),
        ]);
        let _cleanup = StoreCleanup::new(&ws);
        append_items(&ws, &ws.config.sources[0], &[item("from a", "PROJ-1")]).unwrap();
        append_items(&ws, &ws.config.sources[1], &[item("from b", "PROJ-1")]).unwrap();

        let slug = source_slug(&ws.config.sources[1]);
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

    fn workspace(sources: Vec<SourceConfig>) -> Workspace {
        Workspace {
            id: format!(
                "test-{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ),
            path: "work.toml".into(),
            config: WorkspaceConfig { sources },
        }
    }

    fn jira_source(id: &str, site: &str, jql: &str) -> SourceConfig {
        SourceConfig {
            id: id.into(),
            source: SourceKind::Jira {
                site: site.into(),
                email_env: "JIRA_EMAIL".into(),
                token_env: "JIRA_API_TOKEN".into(),
                credentials: None,
                jql: jql.into(),
                limit: 50,
                fields: vec![],
                field_map: Default::default(),
                status_map: Default::default(),
            },
            actions: vec![],
        }
    }

    fn item(title: &str, id: &str) -> Item {
        Item {
            id: id.into(),
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
            success,
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

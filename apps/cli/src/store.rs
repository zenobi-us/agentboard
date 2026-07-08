use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Command as ProcessCommand, Stdio},
};

use agentboard_core::model::{ActionAttempt, Item, SourceConfig, SourceKind, Workspace};
use anyhow::{anyhow, Context, Result};
use fs4::{FileExt, TryLockError};
use serde_json::json;

use crate::{
    adapters::collect_items,
    config::{items_path, source_dir, source_slug, store_root},
};

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
    let mut f = append_file(source_dir(ws, &source.id).join("actions.jsonl"))?;
    writeln!(f, "{}", serde_json::to_string(attempt)?)?;
    Ok(())
}

fn append_file(path: PathBuf) -> Result<File> {
    fs::create_dir_all(path.parent().unwrap())?;
    Ok(OpenOptions::new().create(true).append(true).open(path)?)
}

/// Return the latest observed item per item id across configured sources.
pub fn latest_items(ws: &Workspace) -> Result<HashMap<String, Item>> {
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
            map.insert(item_key(&slug, &item.id), item);
        }
    }
    Ok(map)
}

/// Return every stored action attempt for the workspace.
pub fn all_actions(ws: &Workspace) -> Result<Vec<ActionAttempt>> {
    let mut out = Vec::new();
    for source in &ws.config.sources {
        let path = source_dir(ws, &source.id).join("actions.jsonl");
        if !path.exists() {
            continue;
        }
        for line in BufReader::new(File::open(path)?).lines() {
            out.push(serde_json::from_str(&line?)?);
        }
    }
    Ok(out)
}

/// Return identity keys for successful actions, used to skip already-completed work.
pub fn successful_actions(ws: &Workspace, source_id: &str) -> Result<HashSet<String>> {
    let path = source_dir(ws, source_id).join("actions.jsonl");
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
    let mut items: Vec<_> = latest_items(ws)?.into_values().collect();
    items.sort_by(|a, b| a.id.cmp(&b.id));
    let actions = all_actions(ws)?;
    if as_json {
        let rows: Vec<_> = items
            .into_iter()
            .map(|item| json!({ "item": item, "action_state": action_state(&actions, &item.id) }))
            .collect();
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        for item in items {
            println!(
                "{}\t{}\t{}\t{}",
                item.id,
                item.status,
                action_state(&actions, &item.id),
                item.title
            );
        }
    }
    Ok(())
}

// Derive display state from action attempts for one item.
fn action_state(actions: &[ActionAttempt], item_id: &str) -> &'static str {
    let mut saw_action = false;
    let mut saw_failure = false;
    for action in actions.iter().filter(|a| a.item_id == item_id) {
        saw_action = true;
        saw_failure |= !action.success;
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
pub fn show_item(ws: &Workspace, item_id: &str, as_json: bool) -> Result<()> {
    let matches: Vec<_> = latest_items(ws)?
        .into_values()
        .filter(|item| item.id == item_id)
        .collect();
    let item = match matches.as_slice() {
        [] => return Err(anyhow!("item {item_id} not found")),
        [item] => item.clone(),
        _ => {
            return Err(anyhow!(
                "item {item_id} is ambiguous across Store item buckets"
            ))
        }
    };
    let actions: Vec<_> = all_actions(ws)?
        .into_iter()
        .filter(|a| a.item_id == item_id)
        .collect();
    if as_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({"item": item, "actions": actions}))?
        );
    } else {
        println!("{}\n{}\n{}\n{}", item.id, item.title, item.status, item.url);
        for a in actions {
            println!(
                "action#{} {} success={}",
                a.source_action_index, a.uses, a.success
            );
        }
    }
    Ok(())
}

/// Validate config, store writability, source reachability, and required commands.
pub async fn doctor(ws: &Workspace) -> Result<()> {
    crate::config::validate_config(&ws.config)?;
    let root = store_root(ws);
    fs::create_dir_all(&root)?;
    let probe = root.join(".doctor-write-test");
    fs::write(&probe, b"ok")?;
    fs::remove_file(probe)?;
    for source in &ws.config.sources {
        match &source.source {
            SourceKind::Qmd { .. } => command_exists("qmd")?,
            SourceKind::Jira { .. } => {}
        }
        let _ = collect_items(source).await?;
    }
    for source in &ws.config.sources {
        for action in &source.actions {
            if action.uses == "agentboard/create-worktree" {
                command_exists("git")?;
            }
        }
    }
    println!("ok {}", ws.id);
    Ok(())
}

fn command_exists(cmd: &str) -> Result<()> {
    ProcessCommand::new(cmd)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|_| ())
        .with_context(|| format!("required command {cmd} not found"))
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
                map: Default::default(),
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

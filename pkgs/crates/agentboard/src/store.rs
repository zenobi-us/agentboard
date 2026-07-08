use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Command as ProcessCommand, Stdio},
};

use anyhow::{anyhow, Context, Result};
use fs4::{FileExt, TryLockError};
use serde_json::json;

use crate::{
    config::{source_dir, store_root},
    model::{ActionAttempt, Item, SourceConfig, Workspace},
    sources::collect_items,
};

pub struct Lock(File);

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.0);
    }
}

pub fn acquire_lock(ws: &Workspace) -> Result<Lock> {
    let path = store_root(ws).join("run.lock");
    fs::create_dir_all(path.parent().unwrap())?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
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

pub fn append_items(ws: &Workspace, source: &SourceConfig, items: &[Item]) -> Result<()> {
    let mut f = append_file(source_dir(ws, &source.id).join("items.jsonl"))?;
    for item in items {
        writeln!(f, "{}", serde_json::to_string(item)?)?;
    }
    Ok(())
}

pub fn append_action(ws: &Workspace, source: &SourceConfig, attempt: &ActionAttempt) -> Result<()> {
    let mut f = append_file(source_dir(ws, &source.id).join("actions.jsonl"))?;
    writeln!(f, "{}", serde_json::to_string(attempt)?)?;
    Ok(())
}

fn append_file(path: PathBuf) -> Result<File> {
    fs::create_dir_all(path.parent().unwrap())?;
    Ok(OpenOptions::new().create(true).append(true).open(path)?)
}

pub fn latest_items(ws: &Workspace) -> Result<HashMap<String, Item>> {
    let mut map = HashMap::new();
    for source in &ws.config.sources {
        let path = source_dir(ws, &source.id).join("items.jsonl");
        if !path.exists() {
            continue;
        }
        for line in BufReader::new(File::open(path)?).lines() {
            let item: Item = serde_json::from_str(&line?)?;
            map.insert(item.id.clone(), item);
        }
    }
    Ok(map)
}

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

pub fn show_item(ws: &Workspace, item_id: &str, as_json: bool) -> Result<()> {
    let item = latest_items(ws)?
        .remove(item_id)
        .ok_or_else(|| anyhow!("item {item_id} not found"))?;
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

pub async fn doctor(ws: &Workspace) -> Result<()> {
    crate::config::validate_config(&ws.config)?;
    let root = store_root(ws);
    fs::create_dir_all(&root)?;
    let probe = root.join(".doctor-write-test");
    fs::write(&probe, b"ok")?;
    fs::remove_file(probe)?;
    for source in &ws.config.sources {
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

pub fn action_key(source_id: &str, item_id: &str, idx: usize, hash: &str) -> String {
    format!("{source_id}\0{item_id}\0{idx}\0{hash}")
}

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    env,
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, Stdio},
    thread,
    time::Duration,
};

use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;
use clap::{Parser, Subcommand};
use minijinja::{context, Environment};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const STDOUT_LIMIT: usize = 64 * 1024;

#[derive(Debug, Parser)]
#[command(name = "agentboard")]
#[command(about = "Collect task-tracking items into local agent work queues")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Execute one workspace run.
    Run {
        workspace: String,
        #[arg(long)]
        dry_run: bool,
    },
    /// Repeatedly run one workspace.
    Watch {
        workspace: String,
        #[arg(long, default_value = "60s")]
        interval: String,
    },
    /// List latest stored items.
    List {
        workspace: String,
        #[arg(long)]
        json: bool,
    },
    /// Show one latest stored item and action attempts.
    Show {
        workspace: String,
        item_id: String,
        #[arg(long)]
        json: bool,
    },
    /// Validate workspace and local environment.
    Doctor { workspace: String },
    /// Print workspace JSON Schema.
    Schema,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct WorkspaceConfig {
    sources: Vec<SourceConfig>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SourceConfig {
    id: String,
    query: Option<String>,
    source: SourceKind,
    #[serde(default)]
    actions: Vec<ActionConfig>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum SourceKind {
    Markdown { path: String },
}

#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone)]
#[serde(deny_unknown_fields)]
struct ActionConfig {
    uses: String,
    #[serde(default, rename = "with")]
    inputs: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Item {
    id: String,
    title: String,
    status: String,
    url: String,
    source_id: String,
    source_kind: String,
    raw: Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct ActionAttempt {
    ts: String,
    source_id: String,
    item_id: String,
    source_action_index: usize,
    uses: String,
    rendered_action_hash: String,
    success: bool,
    stdout: String,
    stderr: String,
    message: Option<String>,
}

#[derive(Debug)]
struct Workspace {
    id: String,
    path: PathBuf,
    config: WorkspaceConfig,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Run { workspace, dry_run } => run_once(&load_workspace(&workspace)?, dry_run),
        Command::Watch {
            workspace,
            interval,
        } => {
            let ws = load_workspace(&workspace)?;
            let delay = parse_duration(&interval)?;
            let _lock = acquire_lock(&ws)?;
            loop {
                run_sources(&ws, false)?;
                thread::sleep(delay);
            }
        }
        Command::List { workspace, json } => list_items(&load_workspace(&workspace)?, json),
        Command::Show {
            workspace,
            item_id,
            json,
        } => show_item(&load_workspace(&workspace)?, &item_id, json),
        Command::Doctor { workspace } => doctor(&load_workspace(&workspace)?),
        Command::Schema => {
            println!(
                "{}",
                serde_json::to_string_pretty(&schemars::schema_for!(WorkspaceConfig))?
            );
            Ok(())
        }
    }
}

fn load_workspace(input: &str) -> Result<Workspace> {
    let path = if input.ends_with(".toml") || input.contains('/') {
        expand_path(input)
    } else {
        config_home()
            .join("agentboard")
            .join(format!("{input}.toml"))
    };
    let text =
        fs::read_to_string(&path).with_context(|| format!("read workspace {}", path.display()))?;
    let config: WorkspaceConfig = toml::from_str(&text)?;
    validate_config(&config)?;
    let id = if input.ends_with(".toml") || input.contains('/') {
        let canon = fs::canonicalize(&path)?;
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("workspace");
        format!("{stem}-{}", short_hash(&canon.display().to_string()))
    } else {
        input.to_string()
    };
    Ok(Workspace { id, path, config })
}

fn validate_config(config: &WorkspaceConfig) -> Result<()> {
    let mut ids = HashSet::new();
    for src in &config.sources {
        if !ids.insert(&src.id) {
            bail!("duplicate source id {}", src.id);
        }
        if src.id.trim().is_empty() {
            bail!("source id cannot be empty");
        }
        if let Some(q) = &src.query {
            parse_query(q).with_context(|| format!("invalid query for source {}", src.id))?;
        }
        for action in &src.actions {
            match action.uses.as_str() {
                "agentboard/create-worktree" => require_inputs(action, &["repo", "root", "branch"]),
                "agentboard/run-cmd" => require_inputs(action, &["cmd"]),
                other if other.starts_with("agentboard/") => {
                    bail!("unknown built-in action {other}")
                }
                other => bail!("unknown action {other}"),
            }?;
        }
    }
    Ok(())
}

fn require_inputs(action: &ActionConfig, keys: &[&str]) -> Result<()> {
    for key in keys {
        if !action.inputs.contains_key(*key) {
            bail!("{} requires input {key}", action.uses);
        }
    }
    Ok(())
}

fn run_once(ws: &Workspace, dry_run: bool) -> Result<()> {
    let _lock = if dry_run {
        None
    } else {
        Some(acquire_lock(ws)?)
    };
    run_sources(ws, dry_run)
}

fn run_sources(ws: &Workspace, dry_run: bool) -> Result<()> {
    let mut failed = false;
    for source in &ws.config.sources {
        match run_source(ws, source, dry_run) {
            Ok(ok) => failed |= !ok,
            Err(err) => {
                eprintln!("source {} failed: {err:#}", source.id);
                failed = true;
            }
        }
    }
    if failed {
        bail!("run completed with failures");
    }
    Ok(())
}

struct Lock(PathBuf);
impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn acquire_lock(ws: &Workspace) -> Result<Lock> {
    let path = store_root(ws).join("run.lock");
    fs::create_dir_all(path.parent().unwrap())?;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .with_context(|| format!("workspace lock exists at {}", path.display()))?;
    Ok(Lock(path))
}

fn run_source(ws: &Workspace, source: &SourceConfig, dry_run: bool) -> Result<bool> {
    let mut items = collect_items(source)?;
    items.sort_by_key(|item| item.raw["path"].as_str().unwrap_or("").to_string());
    if let Some(q) = &source.query {
        let expr = parse_query(q)?;
        items.retain(|item| eval_query(&expr, &item.raw["frontmatter"]));
    }
    if !dry_run {
        append_items(ws, source, &items)?;
    }
    let successes = successful_actions(ws, &source.id)?;
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

fn collect_items(source: &SourceConfig) -> Result<Vec<Item>> {
    match &source.source {
        SourceKind::Markdown { path } => collect_markdown(&source.id, path),
    }
}

fn collect_markdown(source_id: &str, path: &str) -> Result<Vec<Item>> {
    let root = expand_path(path);
    let mut files = Vec::new();
    collect_md_files(&root, &mut files)?;
    files.sort();
    let mut ids = HashSet::new();
    let mut out = Vec::new();
    for file in files {
        let text = fs::read_to_string(&file)?;
        let (frontmatter, body) =
            parse_frontmatter(&text).with_context(|| format!("parse {}", file.display()))?;
        let id = str_field(&frontmatter, "id")?;
        if !ids.insert(id.clone()) {
            bail!("duplicate item id {id} in source {source_id}");
        }
        let title = str_field(&frontmatter, "title")?;
        let status = str_field(&frontmatter, "status")?;
        let canonical = fs::canonicalize(&file)?;
        let url = frontmatter
            .get("url")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("file://{}", canonical.display()));
        out.push(Item {
            id,
            title,
            status,
            url,
            source_id: source_id.to_string(),
            source_kind: "markdown".to_string(),
            raw: json!({ "frontmatter": frontmatter, "body": body, "path": canonical }),
        });
    }
    Ok(out)
}

fn collect_md_files(path: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(path).with_context(|| format!("read dir {}", path.display()))? {
        let entry = entry?;
        let p = entry.path();
        if p.is_dir() {
            collect_md_files(&p, out)?;
        } else if p.extension().and_then(|s| s.to_str()) == Some("md") {
            out.push(p);
        }
    }
    Ok(())
}

fn parse_frontmatter(text: &str) -> Result<(Value, String)> {
    let rest = text
        .strip_prefix("---\n")
        .ok_or_else(|| anyhow!("missing YAML frontmatter"))?;
    let (yaml, body) = rest
        .split_once("\n---\n")
        .ok_or_else(|| anyhow!("unclosed YAML frontmatter"))?;
    Ok((serde_yaml::from_str(yaml)?, body.to_string()))
}

fn str_field(v: &Value, key: &str) -> Result<String> {
    v.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| anyhow!("frontmatter {key} must be a string"))
}

struct RenderedAction {
    inputs: BTreeMap<String, String>,
    hash: String,
}

fn render_action(
    ws: &Workspace,
    source: &SourceConfig,
    item: &Item,
    idx: usize,
    action: &ActionConfig,
) -> Result<RenderedAction> {
    let mut env = Environment::new();
    env.add_filter("slugify", slugify);
    let mut inputs = BTreeMap::new();
    for (key, value) in &action.inputs {
        let rendered = env.render_str(
            value,
            context! {
                workspace => json!({"id": ws.id, "path": ws.path}),
                source => source,
                item => item,
                action => json!({"uses": action.uses, "index": idx}),
            },
        )?;
        inputs.insert(key.clone(), expand_vars(&rendered));
    }
    let hash = hash_json(&json!({"uses": action.uses, "with": inputs}));
    Ok(RenderedAction { inputs, hash })
}

fn slugify(s: String) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

struct ActionRun {
    success: bool,
    stdout: String,
    stderr: String,
    message: Option<String>,
}
fn execute_action(
    ws: &Workspace,
    source: &SourceConfig,
    item: &Item,
    idx: usize,
    action: &ActionConfig,
    rendered: RenderedAction,
) -> Result<ActionAttempt> {
    let run = match action.uses.as_str() {
        "agentboard/run-cmd" => run_cmd(ws, source, item, &rendered.inputs)?,
        "agentboard/create-worktree" => match create_worktree(&rendered.inputs) {
            Ok((stdout, stderr)) => ActionRun {
                success: true,
                stdout,
                stderr,
                message: None,
            },
            Err(err) => ActionRun {
                success: false,
                stdout: String::new(),
                stderr: format!("{err:#}"),
                message: Some(err.to_string()),
            },
        },
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

fn run_cmd(
    ws: &Workspace,
    source: &SourceConfig,
    item: &Item,
    inputs: &BTreeMap<String, String>,
) -> Result<ActionRun> {
    let cmd = inputs.get("cmd").unwrap();
    let mut c = ProcessCommand::new("sh");
    c.arg("-c")
        .arg(cmd)
        .stdin(Stdio::null())
        .env("AGENTBOARD_WORKSPACE_ID", &ws.id)
        .env("AGENTBOARD_SOURCE_ID", &source.id)
        .env("AGENTBOARD_ITEM_ID", &item.id);
    if let Some(cwd) = inputs.get("cwd") {
        c.current_dir(cwd);
    }
    let out = c.output()?;
    let success = out.status.success();
    Ok(ActionRun {
        success,
        stdout: cap(&out.stdout),
        stderr: cap(&out.stderr),
        message: (!success).then(|| format!("command exited with {}", out.status)),
    })
}

fn create_worktree(inputs: &BTreeMap<String, String>) -> Result<(String, String)> {
    let repo = inputs.get("repo").unwrap();
    let root = inputs.get("root").unwrap();
    let branch = inputs.get("branch").unwrap();
    if Path::new(root).exists() {
        let out = ProcessCommand::new("git")
            .args(["-C", root, "branch", "--show-current"])
            .output()?;
        let current = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if out.status.success() && current == *branch {
            return Ok((format!("reused {root}\n"), String::new()));
        }
        bail!("{} exists but is not worktree for branch {}", root, branch);
    }
    let exists = ProcessCommand::new("git")
        .args(["-C", repo, "rev-parse", "--verify", branch])
        .output()?
        .status
        .success();
    let mut cmd = ProcessCommand::new("git");
    cmd.arg("-C").arg(repo).arg("worktree").arg("add");
    if exists {
        cmd.arg(root).arg(branch);
    } else {
        cmd.arg("-b").arg(branch).arg(root);
    }
    let out = cmd.output()?;
    if !out.status.success() {
        bail!(
            "git worktree failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok((cap(&out.stdout), cap(&out.stderr)))
}

fn append_items(ws: &Workspace, source: &SourceConfig, items: &[Item]) -> Result<()> {
    let mut f = append_file(source_dir(ws, &source.id).join("items.jsonl"))?;
    for item in items {
        writeln!(f, "{}", serde_json::to_string(item)?)?;
    }
    Ok(())
}

fn append_action(ws: &Workspace, source: &SourceConfig, attempt: &ActionAttempt) -> Result<()> {
    let mut f = append_file(source_dir(ws, &source.id).join("actions.jsonl"))?;
    writeln!(f, "{}", serde_json::to_string(attempt)?)?;
    Ok(())
}

fn append_file(path: PathBuf) -> Result<File> {
    fs::create_dir_all(path.parent().unwrap())?;
    Ok(OpenOptions::new().create(true).append(true).open(path)?)
}

fn latest_items(ws: &Workspace) -> Result<HashMap<String, Item>> {
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

fn all_actions(ws: &Workspace) -> Result<Vec<ActionAttempt>> {
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

fn successful_actions(ws: &Workspace, source_id: &str) -> Result<HashSet<String>> {
    Ok(all_actions(ws)?
        .into_iter()
        .filter(|a| a.source_id == source_id && a.success)
        .map(|a| {
            action_key(
                &a.source_id,
                &a.item_id,
                a.source_action_index,
                &a.rendered_action_hash,
            )
        })
        .collect())
}

fn list_items(ws: &Workspace, as_json: bool) -> Result<()> {
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

fn show_item(ws: &Workspace, item_id: &str, as_json: bool) -> Result<()> {
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

fn doctor(ws: &Workspace) -> Result<()> {
    validate_config(&ws.config)?;
    let root = store_root(ws);
    fs::create_dir_all(&root)?;
    let probe = root.join(".doctor-write-test");
    fs::write(&probe, b"ok")?;
    fs::remove_file(probe)?;
    for source in &ws.config.sources {
        let _ = collect_items(source)?;
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

#[derive(Debug, Clone, PartialEq)]
enum Expr {
    Term(String, String),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
}

fn parse_query(s: &str) -> Result<Expr> {
    let normalized = s.replace("NOT ", "-");
    let condition = search_query_parser::parse_query_to_condition(&normalized)
        .map_err(|err| anyhow!("invalid query: {err}"))?;
    expr_from_condition(condition)
}

fn expr_from_condition(condition: search_query_parser::Condition) -> Result<Expr> {
    use search_query_parser::{Condition, Operator};

    match condition {
        Condition::None => bail!("query cannot be empty"),
        Condition::Keyword(token) | Condition::PhraseKeyword(token) => term_from_token(&token),
        Condition::Not(inner) => Ok(Expr::Not(Box::new(expr_from_condition(*inner)?))),
        Condition::Operator(Operator::And, items) => fold_conditions(items, Expr::And),
        Condition::Operator(Operator::Or, items) => fold_conditions(items, Expr::Or),
    }
}

fn fold_conditions(
    items: Vec<search_query_parser::Condition>,
    combine: fn(Box<Expr>, Box<Expr>) -> Expr,
) -> Result<Expr> {
    let mut items = items.into_iter().map(expr_from_condition);
    let first = items
        .next()
        .ok_or_else(|| anyhow!("query group cannot be empty"))??;
    items.try_fold(first, |acc, item| {
        Ok(combine(Box::new(acc), Box::new(item?)))
    })
}

fn term_from_token(token: &str) -> Result<Expr> {
    let (field, value) = token
        .split_once(':')
        .ok_or_else(|| anyhow!("fieldless terms are not allowed"))?;
    if field.is_empty() || value.is_empty() {
        bail!("query terms must be field:value");
    }
    Ok(Expr::Term(field.to_string(), value.to_string()))
}

fn eval_query(e: &Expr, fm: &Value) -> bool {
    match e {
        Expr::Term(k, v) => fm.get(k).is_some_and(|actual| value_matches(actual, v)),
        Expr::And(a, b) => eval_query(a, fm) && eval_query(b, fm),
        Expr::Or(a, b) => eval_query(a, fm) || eval_query(b, fm),
        Expr::Not(a) => !eval_query(a, fm),
    }
}

fn value_matches(actual: &Value, expected: &str) -> bool {
    match actual {
        Value::String(s) => s == expected,
        Value::Array(items) => items.iter().any(|v| value_matches(v, expected)),
        Value::Number(n) => n.to_string() == expected,
        Value::Bool(b) => b.to_string() == expected,
        _ => false,
    }
}

fn config_home() -> PathBuf {
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".config"))
}
fn data_home() -> PathBuf {
    env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".local/share"))
}
fn home_dir() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}
fn store_root(ws: &Workspace) -> PathBuf {
    data_home().join("agentboard").join(&ws.id)
}
fn source_dir(ws: &Workspace, source_id: &str) -> PathBuf {
    store_root(ws).join("sources").join(source_id)
}
fn action_key(source_id: &str, item_id: &str, idx: usize, hash: &str) -> String {
    format!("{source_id}\0{item_id}\0{idx}\0{hash}")
}
fn hash_json(v: &Value) -> String {
    let mut h = Sha256::new();
    h.update(serde_json::to_vec(v).unwrap());
    hex::encode(h.finalize())
}
fn short_hash(s: &str) -> String {
    hex::encode(Sha256::digest(s.as_bytes()))[..12].to_string()
}
fn cap(bytes: &[u8]) -> String {
    String::from_utf8_lossy(&bytes[..bytes.len().min(STDOUT_LIMIT)]).to_string()
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

fn expand_path(s: &str) -> PathBuf {
    PathBuf::from(expand_vars(s))
}
fn expand_vars(s: &str) -> String {
    let mut out = if let Some(rest) = s.strip_prefix("~/") {
        home_dir().join(rest).display().to_string()
    } else {
        s.to_string()
    };
    for (k, v) in env::vars() {
        out = out
            .replace(&format!("${k}"), &v)
            .replace(&format!("${{{k}}}"), &v);
    }
    out
}
fn parse_duration(s: &str) -> Result<Duration> {
    let secs = s.strip_suffix('s').unwrap_or(s).parse()?;
    Ok(Duration::from_secs(secs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_matches_scalars_and_arrays() {
        let q = parse_query("status:ready AND (priority:high OR labels:agent)").unwrap();
        let fm = json!({"status":"ready", "priority":"low", "labels":["agent"]});
        assert!(eval_query(&q, &fm));
    }

    #[test]
    fn fieldless_query_is_invalid() {
        assert!(parse_query("ready").is_err());
    }

    #[test]
    fn parses_markdown_frontmatter() {
        let (fm, body) =
            parse_frontmatter("---\nid: AB-1\ntitle: Do it\nstatus: ready\n---\nBody").unwrap();
        assert_eq!(fm["id"], "AB-1");
        assert_eq!(body, "Body");
    }

    #[test]
    fn slug_filter_is_path_safe() {
        assert_eq!(slugify("Fix Login!".into()), "fix-login");
    }
}

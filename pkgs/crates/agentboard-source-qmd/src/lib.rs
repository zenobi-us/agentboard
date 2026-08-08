use std::{
    collections::HashSet,
    process::{Command as ProcessCommand, Stdio},
    thread,
    time::Duration,
};

use anyhow::{anyhow, bail, Context, Result};
use command_group::{AsyncCommandGroup, CommandGroup};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use agentboard_core::{
    model::{FieldMap, Item},
    registry::{
        HealthCheck, HealthCheckContext, RuntimeResult, Source, SourceCollection, SourceContext,
        SourceDefinition, SourceFuture,
    },
    CancellationToken,
};

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QmdSourceConfig {
    pub collections: Vec<String>,
    pub query: String,
    #[serde(default = "default_source_limit")]
    pub limit: usize,
    #[serde(default)]
    pub map: FieldMap,
}

pub struct QmdSourceDefinition;

pub struct QmdSource {
    config: QmdSourceConfig,
}

impl SourceDefinition for QmdSourceDefinition {
    const ID: &'static str = "qmd";
    type Config = QmdSourceConfig;
    type Runtime = QmdSource;

    fn build(config: Self::Config) -> RuntimeResult<Self::Runtime> {
        if config.collections.is_empty()
            || config.collections.iter().any(|name| name.trim().is_empty())
        {
            bail!("requires at least one collection");
        }
        if config.query.trim().is_empty() {
            bail!("requires query");
        }
        if config.limit == 0 {
            bail!("limit must be greater than zero");
        }
        Ok(QmdSource { config })
    }
}

impl QmdSource {
    async fn collect_qmd(
        &self,
        source_id: &str,
        cancellation: &CancellationToken,
    ) -> Result<SourceCollection> {
        if cancellation.is_cancelled() {
            return Err(qmd_cancelled());
        }
        let results = qmd_query(
            &self.config.collections,
            &self.config.query,
            self.config.limit,
            cancellation,
        )
        .await?;
        self.collection_from_results_with_cancellation(source_id, results, cancellation)
    }

    #[cfg(test)]
    fn collection_from_results(
        &self,
        source_id: &str,
        results: Vec<Value>,
    ) -> Result<SourceCollection> {
        self.collection_from_results_with_cancellation(
            source_id,
            results,
            &CancellationToken::new(),
        )
    }

    fn collection_from_results_with_cancellation(
        &self,
        source_id: &str,
        results: Vec<Value>,
        cancellation: &CancellationToken,
    ) -> Result<SourceCollection> {
        let mut ids = HashSet::new();
        let mut items = Vec::new();

        for result in results {
            if cancellation.is_cancelled() {
                return Err(qmd_cancelled());
            }
            let doc_ref = doc_ref(&result)?;
            let doc = result
                .get("body")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("qmd result {doc_ref} missing string body"))?;
            let (frontmatter, body) =
                parse_frontmatter(doc).with_context(|| format!("parse qmd document {doc_ref}"))?;
            if cancellation.is_cancelled() {
                return Err(qmd_cancelled());
            }
            let item = normalize_document(
                source_id,
                result,
                doc_ref,
                frontmatter,
                body,
                &self.config.map,
            )?;
            if cancellation.is_cancelled() {
                return Err(qmd_cancelled());
            }
            if !ids.insert(item.id.clone()) {
                bail!("duplicate item id {} in source {source_id}", item.id);
            }
            items.push(item);
        }

        Ok(SourceCollection {
            items,
            available: None,
            limit: self.config.limit,
        })
    }
}

impl Source for QmdSource {
    fn collect<'a>(&'a self, context: &'a SourceContext<'a>) -> SourceFuture<'a> {
        Box::pin(async move {
            self.collect_qmd(context.source_id, &context.cancellation)
                .await
        })
    }

    fn health_checks(&self, context: &HealthCheckContext<'_>) -> Vec<HealthCheck> {
        vec![HealthCheck {
            name: "command qmd".into(),
            result: check_qmd_command(context),
        }]
    }

    fn item_bucket_identity(&self) -> String {
        let mut collections = self.config.collections.clone();
        collections.sort();
        collections.join(",")
    }
}

fn default_source_limit() -> usize {
    50
}

fn normalize_document(
    source_id: &str,
    result: Value,
    doc_ref: String,
    frontmatter: Value,
    body: String,
    map: &FieldMap,
) -> Result<Item> {
    let reference_id = mapped_field(&frontmatter, map.id.as_deref().unwrap_or("id"), "id")?;
    let title = mapped_field(
        &frontmatter,
        map.title.as_deref().unwrap_or("title"),
        "title",
    )?;
    let status = mapped_field(
        &frontmatter,
        map.status.as_deref().unwrap_or("status"),
        "status",
    )?;
    let url = optional_mapped_field(&frontmatter, map.url.as_deref().unwrap_or("url"))
        .unwrap_or_else(|| doc_ref.clone());

    Ok(Item {
        id: doc_ref,
        reference_id,
        title,
        status,
        url,
        source_id: source_id.to_string(),
        source_kind: "qmd".to_string(),
        raw: json!({ "qmd": result, "frontmatter": frontmatter, "body": body }),
    })
}

fn qmd_cancelled() -> anyhow::Error {
    anyhow!("qmd operation cancelled")
}

async fn qmd_query(
    collections: &[String],
    query: &str,
    limit: usize,
    cancellation: &CancellationToken,
) -> Result<Vec<Value>> {
    if cancellation.is_cancelled() {
        return Err(qmd_cancelled());
    }
    let mut command = qmd_query_command(collections, query, limit);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = tokio::process::Command::from(command)
        .group_spawn()
        .map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                anyhow!(
                    "qmd command not found; install QMD or remove qmd sources from this workspace"
                )
            } else {
                err.into()
            }
        })?;
    let stdout = child
        .inner()
        .stdout
        .take()
        .ok_or_else(|| anyhow!("qmd stdout was not captured"))?;
    let stderr = child
        .inner()
        .stderr
        .take()
        .ok_or_else(|| anyhow!("qmd stderr was not captured"))?;
    use tokio::io::AsyncReadExt;
    let mut stdout = stdout;
    let mut stderr = stderr;
    let output = tokio::select! {
        result = async {
            let mut out = Vec::new();
            let mut err = Vec::new();
            let (status, out_result, err_result) = tokio::join!(child.wait(), stdout.read_to_end(&mut out), stderr.read_to_end(&mut err));
            out_result?;
            err_result?;
            Ok::<_, anyhow::Error>(std::process::Output { status: status?, stdout: out, stderr: err })
        } => result?,
        _ = cancellation.cancelled() => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            return Err(qmd_cancelled());
        }
    };
    let out = output;
    if cancellation.is_cancelled() {
        return Err(qmd_cancelled());
    }
    if !out.status.success() {
        bail!("qmd query failed: {}", String::from_utf8_lossy(&out.stderr));
    }
    let results = parse_qmd_results(&String::from_utf8_lossy(&out.stdout))?;
    if cancellation.is_cancelled() {
        return Err(qmd_cancelled());
    }
    Ok(results)
}

fn qmd_query_command(collections: &[String], query: &str, limit: usize) -> ProcessCommand {
    let mut cmd = ProcessCommand::new("qmd");
    cmd.arg("query")
        .arg(query)
        .arg("--format")
        .arg("json")
        .arg("--full")
        .arg("-n")
        .arg(limit.to_string());
    for collection in collections {
        cmd.arg("-c").arg(collection);
    }
    cmd
}

fn check_qmd_command(context: &HealthCheckContext<'_>) -> Result<()> {
    if context.cancellation.is_cancelled() {
        return Err(qmd_cancelled());
    }
    let mut child = ProcessCommand::new("qmd")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .group_spawn()
        .with_context(|| "required command qmd not found")?;
    loop {
        if let Some(status) = child.try_wait()? {
            if !status.success() {
                bail!("required command qmd returned {status}");
            }
            return Ok(());
        }
        if context.cancellation.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(qmd_cancelled());
        }
        thread::sleep(Duration::from_millis(5));
    }
}

fn parse_qmd_results(text: &str) -> Result<Vec<Value>> {
    let value: Value = serde_json::from_str(text).context("parse qmd query JSON")?;
    if let Some(items) = value.as_array() {
        return Ok(items.clone());
    }
    for key in ["results", "documents", "items"] {
        if let Some(items) = value.get(key).and_then(Value::as_array) {
            return Ok(items.clone());
        }
    }
    bail!("qmd query JSON must be an array or contain results/documents/items")
}

fn doc_ref(result: &Value) -> Result<String> {
    for key in ["docid", "doc_id", "id", "uri", "path"] {
        if let Some(s) = result.get(key).and_then(Value::as_str) {
            return Ok(s.to_string());
        }
    }
    bail!("qmd result missing docid/doc_id/id/uri/path")
}

pub fn parse_frontmatter(text: &str) -> Result<(Value, String)> {
    let rest = text
        .strip_prefix("---\n")
        .ok_or_else(|| anyhow!("missing YAML frontmatter"))?;
    let (yaml, body) = rest
        .split_once("\n---\n")
        .ok_or_else(|| anyhow!("unclosed YAML frontmatter"))?;
    Ok((yaml_serde::from_str(yaml)?, body.to_string()))
}

fn mapped_field(frontmatter: &Value, path: &str, name: &str) -> Result<String> {
    optional_mapped_field(frontmatter, path)
        .ok_or_else(|| anyhow!("frontmatter mapping {name}={path} must resolve to a string"))
}

fn optional_mapped_field(frontmatter: &Value, path: &str) -> Option<String> {
    let mut value = frontmatter;
    for part in path.split('.') {
        value = value.get(part)?;
    }
    value.as_str().map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentboard_core::{
        registry::{
            HealthCheckContext, RawConfig, Registry, Source, SourceContext, SourceDefinition,
        },
        CancellationToken,
    };
    use std::{
        env, fs,
        future::Future,
        sync::{Arc, Mutex},
        task::{Context as TaskContext, Poll, Waker},
        time::{SystemTime, UNIX_EPOCH},
    };

    static PATH_LOCK: Mutex<()> = Mutex::new(());

    fn poll_ready<T>(future: impl Future<Output = T>) -> T {
        let mut context = TaskContext::from_waker(Waker::noop());
        let mut future = Box::pin(future);
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("future unexpectedly pending"),
        }
    }

    fn config(collections: &[&str]) -> QmdSourceConfig {
        serde_json::from_value(json!({
            "collections": collections,
            "query": "status:ready"
        }))
        .unwrap()
    }

    #[test]
    fn registers_qmd_config_schema() {
        let mut registry = Registry::new();
        registry.add_source::<QmdSourceDefinition>().unwrap();

        let registration = registry.sources().next().unwrap();
        let schema = serde_json::to_value(registration.schema()).unwrap();

        assert_eq!(registration.id(), "qmd");
        assert!(schema["properties"]["collections"].is_object());
        assert!(schema["properties"]["query"].is_object());
        assert_eq!(schema["properties"]["limit"]["default"], 50);
        assert_eq!(schema["additionalProperties"], false);
        let required = schema["required"].as_array().unwrap();
        for field in ["collections", "query"] {
            assert!(required.iter().any(|value| value == field));
        }
        assert_eq!(config(&["tasks"]).limit, 50);

        let source = registry
            .build_source(
                "qmd",
                serde_json::from_value::<RawConfig>(json!({
                    "collections": ["tasks"],
                    "query": "status:ready"
                }))
                .unwrap(),
            )
            .unwrap();
        assert_eq!(source.item_bucket_identity(), "tasks");
        assert!(registry
            .build_source(
                "qmd",
                serde_json::from_value::<RawConfig>(json!({
                    "collections": ["tasks"],
                    "query": "status:ready",
                    "extra": true
                }))
                .unwrap(),
            )
            .is_err());
    }

    #[test]
    fn deserializes_existing_qmd_toml_fields() {
        let config: QmdSourceConfig = toml::from_str(
            r#"
                collections = ["tasks"]
                query = "intent: Find ready work items\nlex: status ready"
                limit = 50
                map = { id = "agentboard.id", status = "workflow.status" }
            "#,
        )
        .unwrap();

        assert!(QmdSourceDefinition::build(config).is_ok());
    }

    #[tokio::test]
    async fn qmd_query_does_not_start_when_cancelled() {
        let cancellation = CancellationToken::new();
        cancellation.cancel();

        let error = qmd_query(&["tasks".into()], "status:ready", 50, &cancellation)
            .await
            .unwrap_err();

        assert_eq!(error.to_string(), "qmd operation cancelled");
    }

    #[cfg(unix)]
    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn qmd_query_cancellation_kills_process_group_descendants() {
        use std::os::unix::fs::PermissionsExt;

        let _guard = PATH_LOCK.lock().unwrap();
        let root = env::temp_dir().join(format!(
            "agentboard-qmd-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let qmd = root.join("qmd");
        let child_pid = root.join("child.pid");
        fs::write(
            &qmd,
            "#!/bin/sh\n/bin/sh -c 'sleep 30' &\necho \"$!\" > \"$QMD_CHILD_PID\"\nwait\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&qmd).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&qmd, permissions).unwrap();

        let original_path = env::var_os("PATH");
        let mut paths = vec![root.clone()];
        if let Some(path) = &original_path {
            paths.extend(env::split_paths(path));
        }
        env::set_var("PATH", env::join_paths(paths).unwrap());
        env::set_var("QMD_CHILD_PID", &child_pid);

        let source = Arc::new(QmdSourceDefinition::build(config(&["tasks"])).unwrap());
        let cancellation = CancellationToken::new();
        let task_source = Arc::clone(&source);
        let task_cancellation = cancellation.clone();
        let task = tokio::spawn(async move {
            task_source
                .collect(&SourceContext {
                    source_id: "local",
                    cancellation: task_cancellation,
                })
                .await
        });

        for _ in 0..1_000 {
            if child_pid.exists() {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert!(child_pid.exists(), "QMD descendant did not start");

        cancellation.cancel();
        let error = task.await.unwrap().unwrap_err();
        assert_eq!(error.to_string(), "qmd operation cancelled");

        let pid = fs::read_to_string(&child_pid).unwrap();
        let pid = pid.trim();
        for _ in 0..100 {
            let alive = ProcessCommand::new("/bin/kill")
                .args(["-0", pid])
                .stderr(Stdio::null())
                .status()
                .unwrap()
                .success();
            if !alive {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert!(!ProcessCommand::new("/bin/kill")
            .args(["-0", pid])
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success());

        match original_path {
            Some(path) => env::set_var("PATH", path),
            None => env::remove_var("PATH"),
        }
        env::remove_var("QMD_CHILD_PID");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn qmd_health_check_does_not_start_when_cancelled() {
        let source = QmdSourceDefinition::build(config(&["tasks"])).unwrap();
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let context = HealthCheckContext {
            source_id: "local",
            cancellation,
        };

        let checks = source.health_checks(&context);

        assert_eq!(checks.len(), 1);
        assert_eq!(
            checks[0].result.as_ref().unwrap_err().to_string(),
            "qmd operation cancelled"
        );
    }

    #[test]
    fn cancelled_collection_does_not_normalize_results() {
        let source = QmdSourceDefinition::build(config(&["tasks"])).unwrap();
        let cancellation = CancellationToken::new();
        cancellation.cancel();

        let error = source
            .collection_from_results_with_cancellation(
                "local",
                vec![json!({
                    "path": "/notes/AB-1.md",
                    "body": "---\nid: AB-1\ntitle: Do it\nstatus: ready\n---\nBody"
                })],
                &cancellation,
            )
            .unwrap_err();

        assert_eq!(error.to_string(), "qmd operation cancelled");
    }

    #[test]
    fn validates_qmd_config_without_side_effects() {
        assert!(QmdSourceDefinition::build(config(&["tasks"])).is_ok());
        assert!(QmdSourceDefinition::build(config(&[])).is_err());
        assert!(QmdSourceDefinition::build(config(&[" "])).is_err());

        let mut missing_query = config(&["tasks"]);
        missing_query.query.clear();
        assert!(QmdSourceDefinition::build(missing_query).is_err());

        let mut zero_limit = config(&["tasks"]);
        zero_limit.limit = 0;
        assert!(QmdSourceDefinition::build(zero_limit).is_err());
    }

    #[test]
    fn reports_qmd_collection_metadata_and_stable_bucket_identity() {
        let source = QmdSourceDefinition::build(config(&["work", "tasks"])).unwrap();
        let reordered = QmdSourceDefinition::build(config(&["tasks", "work"])).unwrap();
        let result = json!({
            "path": "/notes/AB-1.md",
            "body": "---\nid: AB-1\ntitle: Do it\nstatus: ready\n---\nBody"
        });
        let collection = source
            .collection_from_results("local", vec![result.clone()])
            .unwrap();

        assert_eq!(source.item_bucket_identity(), "tasks,work");
        assert_eq!(
            source.item_bucket_identity(),
            reordered.item_bucket_identity()
        );
        assert_eq!(collection.items.len(), 1);
        assert_eq!(collection.available, None);
        assert_eq!(collection.limit, 50);
        assert_eq!(collection.items[0].raw["qmd"], result);
        assert_eq!(collection.items[0].raw["body"], "Body");

        for result_without_inline_body in [
            json!({"path": "/notes/missing.md"}),
            json!({"path": "/notes/not-string.md", "body": 42}),
        ] {
            assert!(source
                .collection_from_results("local", vec![result_without_inline_body])
                .unwrap_err()
                .to_string()
                .contains("missing string body"));
        }

        assert!(source
            .collection_from_results(
                "local",
                vec![
                    json!({
                        "path": "/notes/AB-1.md",
                        "body": "---\nid: AB-1\ntitle: One\nstatus: ready\n---\nOne"
                    }),
                    json!({
                        "path": "/notes/AB-1.md",
                        "body": "---\nid: AB-2\ntitle: Two\nstatus: ready\n---\nTwo"
                    }),
                ],
            )
            .is_err());
    }

    #[test]
    fn qmd_health_check_uses_collection_path() {
        let _guard = PATH_LOCK.lock().unwrap();
        let path = env::var_os("PATH");
        env::set_var("PATH", "");
        let source =
            QmdSourceDefinition::build(config(&["agentboard-health-check-missing"])).unwrap();
        let context = HealthCheckContext {
            source_id: "local",
            cancellation: CancellationToken::new(),
        };
        let result = poll_ready(source.health_check(&context));
        match path {
            Some(path) => env::set_var("PATH", path),
            None => env::remove_var("PATH"),
        }

        assert!(result
            .unwrap_err()
            .to_string()
            .contains("qmd command not found"));
    }

    #[test]
    fn parses_markdown_frontmatter() {
        let (fm, body) =
            parse_frontmatter("---\nid: AB-1\ntitle: Do it\nstatus: ready\n---\nBody").unwrap();
        assert_eq!(fm["id"], "AB-1");
        assert_eq!(body, "Body");
    }

    #[test]
    fn parses_result_arrays_and_wrappers() {
        assert_eq!(parse_qmd_results(r##"[{"docid":"#1"}]"##).unwrap().len(), 1);
        assert_eq!(
            parse_qmd_results(r##"{"results":[{"docid":"#1"}]}"##)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn qmd_query_requests_full_inline_document_bodies() {
        let collections = vec!["work".to_string(), "tasks".to_string()];
        let command = qmd_query_command(&collections, "status:ready", 50);
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            args,
            [
                "query",
                "status:ready",
                "--format",
                "json",
                "--full",
                "-n",
                "50",
                "-c",
                "work",
                "-c",
                "tasks",
            ]
        );
    }

    #[test]
    fn supports_nested_field_mapping() {
        let fm = json!({"agentboard":{"id":"AB-1"}});
        assert_eq!(optional_mapped_field(&fm, "agentboard.id").unwrap(), "AB-1");
    }

    #[test]
    fn normalizes_qmd_document_identity_and_reference() {
        let result = json!({"path": "/notes/AB-1.md"});
        let frontmatter = json!({"id": "AB-1", "title": "Do it", "status": "ready"});

        let item = normalize_document(
            "qmd",
            result,
            "/notes/AB-1.md".into(),
            frontmatter,
            "Body".into(),
            &Default::default(),
        )
        .unwrap();

        assert_eq!(item.id, "/notes/AB-1.md");
        assert_eq!(item.reference_id, "AB-1");
    }

    #[test]
    fn qmd_map_id_changes_reference_not_identity() {
        let result = json!({"path": "/notes/AB-1.md"});
        let frontmatter = json!({
            "id": "AB-1",
            "agentboard": {"reference": "customer-42"},
            "title": "Do it",
            "status": "ready"
        });
        let map = FieldMap {
            id: Some("agentboard.reference".into()),
            ..Default::default()
        };

        let item = normalize_document(
            "qmd",
            result,
            "/notes/AB-1.md".into(),
            frontmatter,
            "Body".into(),
            &map,
        )
        .unwrap();

        assert_eq!(item.id, "/notes/AB-1.md");
        assert_eq!(item.reference_id, "customer-42");
    }
}

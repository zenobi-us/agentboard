use std::{collections::HashSet, process::Command as ProcessCommand};

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};

use crate::{
    model::{FieldMap, Item, SourceConfig, SourceKind},
    sources::SourceAdapter,
};

pub struct QmdSource;

impl SourceAdapter for QmdSource {
    async fn collect(&self, source: &SourceConfig) -> Result<Vec<Item>> {
        match &source.source {
            SourceKind::Qmd {
                collections,
                query,
                limit,
                map,
            } => collect_qmd(&source.id, collections, query, *limit, map),
            _ => bail!("source {} is not qmd", source.id),
        }
    }
}

fn collect_qmd(
    source_id: &str,
    collections: &[String],
    query: &str,
    limit: usize,
    map: &FieldMap,
) -> Result<Vec<Item>> {
    let results = qmd_query(collections, query, limit)?;
    let mut ids = HashSet::new();
    let mut out = Vec::new();

    for result in results {
        let doc_ref = doc_ref(&result)?;
        let doc = qmd_get(&doc_ref)?;
        let (frontmatter, body) =
            parse_frontmatter(&doc).with_context(|| format!("parse qmd document {doc_ref}"))?;
        let id = mapped_field(&frontmatter, map.id.as_deref().unwrap_or("id"), "id")?;
        if !ids.insert(id.clone()) {
            bail!("duplicate item id {id} in source {source_id}");
        }
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

        out.push(Item {
            id,
            title,
            status,
            url,
            source_id: source_id.to_string(),
            source_kind: "qmd".to_string(),
            raw: json!({ "qmd": result, "frontmatter": frontmatter, "body": body }),
        });
    }
    Ok(out)
}

fn qmd_query(collections: &[String], query: &str, limit: usize) -> Result<Vec<Value>> {
    let mut cmd = ProcessCommand::new("qmd");
    cmd.arg("query")
        .arg(query)
        .arg("--format")
        .arg("json")
        .arg("-n")
        .arg(limit.to_string());
    for collection in collections {
        cmd.arg("-c").arg(collection);
    }
    let out = cmd.output().map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            anyhow!("qmd command not found; install QMD or remove qmd sources from this workspace")
        } else {
            err.into()
        }
    })?;
    if !out.status.success() {
        bail!("qmd query failed: {}", String::from_utf8_lossy(&out.stderr));
    }
    parse_qmd_results(&String::from_utf8_lossy(&out.stdout))
}

fn qmd_get(doc_ref: &str) -> Result<String> {
    let out = ProcessCommand::new("qmd")
        .args(["get", doc_ref, "--full"])
        .output()
        .with_context(|| format!("qmd get {doc_ref}"))?;
    if !out.status.success() {
        bail!(
            "qmd get {doc_ref} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
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
    fn supports_nested_field_mapping() {
        let fm = json!({"agentboard":{"id":"AB-1"}});
        assert_eq!(optional_mapped_field(&fm, "agentboard.id").unwrap(), "AB-1");
    }
}

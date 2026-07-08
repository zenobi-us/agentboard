use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};

use crate::{
    config::expand_path,
    model::{Item, SourceConfig, SourceKind},
    sources::SourceAdapter,
};

pub struct MarkdownSource;

impl SourceAdapter for MarkdownSource {
    async fn collect(&self, source: &SourceConfig) -> Result<Vec<Item>> {
        match &source.source {
            SourceKind::Markdown { path } => collect_markdown(&source.id, path),
        }
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

pub fn parse_frontmatter(text: &str) -> Result<(Value, String)> {
    let rest = text
        .strip_prefix("---\n")
        .ok_or_else(|| anyhow!("missing YAML frontmatter"))?;
    let (yaml, body) = rest
        .split_once("\n---\n")
        .ok_or_else(|| anyhow!("unclosed YAML frontmatter"))?;
    Ok((yaml_serde::from_str(yaml)?, body.to_string()))
}

fn str_field(v: &Value, key: &str) -> Result<String> {
    v.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| anyhow!("frontmatter {key} must be a string"))
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
}

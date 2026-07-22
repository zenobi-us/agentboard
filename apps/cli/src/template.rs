//! Renders per-Item Action templates against the complete configured Workspace view.
//!
//! Keeping rendering on serializable config prevents runtime trait objects from
//! leaking into templates and preserves user-facing field names through cutovers.

use std::collections::BTreeMap;

use agentboard_core::{
    model::{ActionConfig, Item, SourceConfig, Workspace},
    RenderedAction,
};
use anyhow::Result;
use minijinja::{context, Environment};
use serde_json::json;

use crate::config::{expand_vars, hash_json};

/// Render an action's input templates and compute its retry identity hash.
pub fn render_action(
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

/// Convert arbitrary text into a conservative path/branch-safe slug.
pub fn slugify(s: String) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use agentboard_core::model::{SourceKind, WorkspaceConfig};
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn slug_filter_is_path_safe() {
        assert_eq!(slugify("Fix Login!".into()), "fix-login");
    }

    #[test]
    fn templates_expose_reference_id_and_complete_source() {
        let action = ActionConfig {
            uses: "agentboard/run-cmd".into(),
            inputs: BTreeMap::from([(
                "cmd".into(),
                "{{ item.reference_id }}|{{ source.id }}|{{ source.source.kind }}|{{ source.source.collections[0] }}|{{ source.actions[0].uses }}".into(),
            )]),
        };
        let source = SourceConfig {
            id: "notes".into(),
            source: SourceKind::Qmd {
                collections: vec!["work".into()],
                query: "status:ready".into(),
                limit: 50,
                map: Default::default(),
            },
            actions: vec![action.clone()],
        };
        let ws = Workspace {
            id: "workspace".into(),
            path: PathBuf::from("/tmp/workspace.toml"),
            config: WorkspaceConfig {
                sources: vec![source.clone()],
            },
            built_sources: vec![],
        };
        let item = Item {
            id: "/notes/AB-1.md".into(),
            reference_id: "AB-1".into(),
            title: "Do it".into(),
            status: "ready".into(),
            url: "/notes/AB-1.md".into(),
            source_id: "notes".into(),
            source_kind: "qmd".into(),
            raw: json!({}),
        };

        let rendered = render_action(&ws, &source, &item, 0, &action).unwrap();

        assert_eq!(
            rendered.inputs["cmd"],
            "AB-1|notes|qmd|work|agentboard/run-cmd"
        );
    }
}

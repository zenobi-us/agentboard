//! Renders per-Item Action templates against the complete configured Workspace view.
//!
//! Keeping rendering on serializable config prevents runtime trait objects from
//! leaking into templates and preserves user-facing field names through cutovers.

use std::collections::BTreeMap;

use agentboard_core::{
    model::{ActionConfig, Item, Workspace, WorkspaceSource},
    RenderedAction,
};
use anyhow::Result;
use minijinja::{context, Environment};
use serde_json::json;

use crate::config::{expand_vars, hash_json};

/// Render an action's input templates and compute its retry identity hash.
pub fn render_action(
    ws: &Workspace,
    source: &WorkspaceSource,
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
                source => &source.configured,
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
    use crate::config::parse_workspace;
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn slug_filter_is_path_safe() {
        assert_eq!(slugify("Fix Login!".into()), "fix-login");
    }

    #[test]
    fn templates_and_action_identity_remain_stable_through_registered_view() {
        let registry = crate::cli::register_builtins().unwrap();
        let parsed = parse_workspace(
            r#"
                [[sources]]
                id = "notes"
                [sources.source]
                kind = "qmd"
                collections = ["work"]
                query = "status:ready"
                [[sources.actions]]
                uses = "agentboard/run-cmd"
                [sources.actions.with]
                cmd = "echo {{ item.reference_id }}"
            "#,
            &registry,
        )
        .unwrap();
        let ws = Workspace {
            id: "workspace".into(),
            path: PathBuf::from("/tmp/workspace.toml"),
            sources: parsed.sources,
        };
        let source = &ws.sources[0];
        let action = &source.configured.actions[0];
        let item = Item {
            id: "/doc/AB-1.md".into(),
            reference_id: "AB-1".into(),
            title: "Do it".into(),
            status: "ready".into(),
            url: "/doc/AB-1.md".into(),
            source_id: "notes".into(),
            source_kind: "qmd".into(),
            raw: json!({}),
        };

        let rendered = render_action(&ws, source, &item, 0, action).unwrap();

        assert_eq!(rendered.inputs["cmd"], "echo AB-1");
        assert_eq!(
            rendered.hash,
            "138a66006ac0ab3ffa344e4b8ad5210839456e5aeed0f0f10226bd44bb5e383d"
        );
        assert_eq!(
            crate::store::action_key(&source.configured.id, &item.id, 0, &rendered.hash),
            concat!(
                "notes\0/doc/AB-1.md",
                "\0",
                "0",
                "\0",
                "138a66006ac0ab3ffa344e4b8ad5210839456e5aeed0f0f10226bd44bb5e383d"
            )
        );
    }

    #[test]
    fn templates_expose_complete_registered_source_shape() {
        let registry = crate::cli::register_builtins().unwrap();
        let parsed = parse_workspace(
            r#"
                [[sources]]
                id = "notes"
                [sources.source]
                kind = "qmd"
                collections = ["work"]
                query = "status:ready"
                [[sources.actions]]
                uses = "agentboard/run-cmd"
                [sources.actions.with]
                cmd = "{{ item.reference_id }}|{{ source.id }}|{{ source.source.kind }}|{{ source.source.collections[0] }}|{{ source.actions[0].uses }}"
            "#,
            &registry,
        )
        .unwrap();
        let ws = Workspace {
            id: "workspace".into(),
            path: PathBuf::from("/tmp/workspace.toml"),
            sources: parsed.sources,
        };
        let source = &ws.sources[0];
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

        let rendered = render_action(&ws, source, &item, 0, &source.configured.actions[0]).unwrap();

        assert_eq!(
            rendered.inputs["cmd"],
            "AB-1|notes|qmd|work|agentboard/run-cmd"
        );
    }
}

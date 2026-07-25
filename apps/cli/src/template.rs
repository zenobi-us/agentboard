//! Renders per-Item Action templates against the complete configured Workspace view.
//!
//! Keeping rendering on serializable config prevents runtime trait objects from
//! leaking into templates and preserves user-facing field names through cutovers.

use std::collections::BTreeMap;

use agentboard_core::{
    model::{ActionConfig, Item, Workspace, WorkspaceSource},
    RenderedAction,
};
use anyhow::{Context, Result};
use minijinja::{context, Environment};
use serde_json::{json, Value};

use crate::config::{expand_vars, hash_json};

fn environment() -> Environment<'static> {
    let mut env = Environment::new();
    env.add_filter("slugify", slugify);
    env
}

/// Validate Action template syntax without requiring an Item render context.
pub fn validate_action_templates(action: &ActionConfig) -> Result<()> {
    let env = environment();
    for (key, value) in &action.inputs {
        env.template_from_str(value)
            .with_context(|| format!("invalid template input {key}"))?;
    }
    Ok(())
}

/// Render an action's input templates and compute its retry identity hash.
pub fn render_action(
    ws: &Workspace,
    source: &WorkspaceSource,
    item: &Item,
    idx: usize,
    action: &ActionConfig,
    actions: &BTreeMap<String, Value>,
) -> Result<RenderedAction> {
    let env = environment();
    let mut inputs = BTreeMap::new();
    for (key, value) in &action.inputs {
        let rendered = env.render_str(
            value,
            context! {
                workspace => json!({"id": ws.id, "path": ws.path}),
                source => &source.configured,
                item => item,
                action => json!({"uses": action.uses, "index": idx}),
                actions => actions,
            },
        )?;
        let rendered = if expands_as_path(&action.uses, key) {
            expand_vars(&rendered)
        } else {
            rendered
        };
        inputs.insert(key.clone(), rendered);
    }
    let hash = hash_json(&json!({"uses": action.uses, "with": inputs}));
    Ok(RenderedAction { inputs, hash })
}

fn expands_as_path(uses: &str, input: &str) -> bool {
    matches!(
        (uses, input),
        ("agentboard/run-cmd", "cwd")
            | ("agentboard/create-worktree", "repo")
            | ("agentboard/create-worktree", "root")
    )
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
    fn unrelated_missing_values_keep_existing_lenient_behavior() {
        let env = environment();

        assert_eq!(
            env.render_str("{{ item.optional }}", context! { item => json!({}) })
                .unwrap(),
            ""
        );
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

        let rendered = render_action(&ws, source, &item, 0, action, &BTreeMap::new()).unwrap();

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
    fn only_path_inputs_expand_agentboard_environment_variables() {
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
                cmd = "printf '%s' \"{{ item.reference_id }}|$PWD|${PWD}\""
                cwd = "$PWD"

                [[sources.actions]]
                uses = "agentboard/create-worktree"
                [sources.actions.with]
                repo = "$PWD/repo"
                root = "${PWD}/worktrees/{{ item.reference_id }}"
                branch = "$PWD/{{ item.reference_id }}"
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
            id: "/doc/AB-1.md".into(),
            reference_id: "AB-1".into(),
            title: "Do it".into(),
            status: "ready".into(),
            url: "/doc/AB-1.md".into(),
            source_id: "notes".into(),
            source_kind: "qmd".into(),
            raw: json!({}),
        };
        let pwd = std::env::var("PWD").unwrap();

        let command = render_action(
            &ws,
            source,
            &item,
            0,
            &source.configured.actions[0],
            &BTreeMap::new(),
        )
        .unwrap();
        let expected_command = BTreeMap::from([
            ("cmd".into(), "printf '%s' \"AB-1|$PWD|${PWD}\"".into()),
            ("cwd".into(), pwd.clone()),
        ]);
        assert_eq!(command.inputs, expected_command);
        assert_eq!(
            command.hash,
            hash_json(&json!({"uses": "agentboard/run-cmd", "with": expected_command}))
        );

        let worktree = render_action(
            &ws,
            source,
            &item,
            1,
            &source.configured.actions[1],
            &BTreeMap::new(),
        )
        .unwrap();
        let expected_worktree = BTreeMap::from([
            ("branch".into(), "$PWD/AB-1".into()),
            ("repo".into(), format!("{pwd}/repo")),
            ("root".into(), format!("{pwd}/worktrees/AB-1")),
        ]);
        assert_eq!(worktree.inputs, expected_worktree);
        assert_eq!(
            worktree.hash,
            hash_json(&json!({"uses": "agentboard/create-worktree", "with": expected_worktree}))
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

        let rendered = render_action(
            &ws,
            source,
            &item,
            0,
            &source.configured.actions[0],
            &BTreeMap::new(),
        )
        .unwrap();

        assert_eq!(
            rendered.inputs["cmd"],
            "AB-1|notes|qmd|work|agentboard/run-cmd"
        );
    }

    #[test]
    fn templates_expose_only_preceding_named_action_inputs() {
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
                id = "issue_worktree"
                uses = "agentboard/create-worktree"
                [sources.actions.with]
                repo = "$PWD/repo"
                root = "$PWD/worktrees/{{ item.reference_id }}"
                branch = "item-{{ item.reference_id }}"

                [[sources.actions]]
                uses = "agentboard/run-cmd"
                [sources.actions.with]
                cmd = "{{ actions.issue_worktree.inputs.root }}"

                [[sources.actions]]
                uses = "agentboard/run-cmd"
                [sources.actions.with]
                cmd = "unnamed"

                [[sources.actions]]
                uses = "agentboard/run-cmd"
                [sources.actions.with]
                cmd = "{{ actions.unnamed.inputs.cmd }}"

                [[sources.actions]]
                uses = "agentboard/run-cmd"
                [sources.actions.with]
                cmd = "{{ actions.future.inputs.cmd }}"

                [[sources.actions]]
                id = "future"
                uses = "agentboard/run-cmd"
                [sources.actions.with]
                cmd = "future"
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
        let first = &source.configured.actions[0];
        let rendered_first = render_action(&ws, source, &item, 0, first, &BTreeMap::new()).unwrap();
        let expected_root = format!("{}/worktrees/AB-1", std::env::var("PWD").unwrap());
        assert_eq!(rendered_first.inputs["root"], expected_root);

        let mut actions = BTreeMap::new();
        actions.insert(
            first.id.clone().unwrap(),
            json!({"inputs": &rendered_first.inputs}),
        );
        let rendered_second = render_action(
            &ws,
            source,
            &item,
            1,
            &source.configured.actions[1],
            &actions,
        )
        .unwrap();
        assert_eq!(rendered_second.inputs["cmd"], expected_root);

        let unnamed = render_action(
            &ws,
            source,
            &item,
            3,
            &source.configured.actions[3],
            &actions,
        )
        .err()
        .expect("unnamed Action should be absent from runtime context");
        assert!(unnamed.to_string().contains("undefined value"));

        let forward = render_action(
            &ws,
            source,
            &item,
            4,
            &source.configured.actions[4],
            &actions,
        )
        .err()
        .expect("forward Action reference should fail");
        assert!(forward.to_string().contains("undefined value"));

        let mut same_inputs_without_id = first.clone();
        same_inputs_without_id.id = None;
        let rendered_without_id = render_action(
            &ws,
            source,
            &item,
            0,
            &same_inputs_without_id,
            &BTreeMap::new(),
        )
        .unwrap();
        assert_eq!(rendered_first.hash, rendered_without_id.hash);
    }
}

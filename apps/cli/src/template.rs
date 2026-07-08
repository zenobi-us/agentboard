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

    #[test]
    fn slug_filter_is_path_safe() {
        assert_eq!(slugify("Fix Login!".into()), "fix-login");
    }
}

//! Composes the Workspace JSON Schema from the same Registry used by loading.
//!
//! Registration-owned schemas remain isolated here so adding a Source or Action
//! cannot create a second closed enum or collide with another crate's definitions.

use std::collections::BTreeMap;

use agentboard_core::registry::{ActionRegistration, Registry, SourceRegistration};
use anyhow::{Context, Result};
use serde_json::{json, Map, Value};

/// Builds one deterministic Draft 7 schema from the process Registry.
///
/// Variants stay inline so discriminators remain obvious to editors, while nested
/// registration definitions move into one namespaced top-level definition map.
pub fn workspace_schema(registry: &Registry) -> Result<Value> {
    let mut definitions = BTreeMap::new();
    let source_variants = registry
        .sources()
        .map(|registration| source_variant(registration, &mut definitions))
        .collect::<Result<Vec<_>>>()?;
    let action_variants = registry
        .actions()
        .map(|registration| action_variant(registration, &mut definitions))
        .collect::<Result<Vec<_>>>()?;

    Ok(json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": "WorkspaceConfig",
        "type": "object",
        "additionalProperties": false,
        "required": ["sources"],
        "properties": {
            "sources": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["id", "source"],
                    "properties": {
                        "id": { "type": "string" },
                        "source": { "oneOf": source_variants },
                        "actions": {
                            "type": "array",
                            "default": [],
                            "items": { "oneOf": action_variants }
                        }
                    }
                }
            }
        },
        "definitions": definitions
    }))
}

/// Adds the literal `kind` discriminator to one registered Source config schema.
fn source_variant(
    registration: &SourceRegistration,
    definitions: &mut BTreeMap<String, Value>,
) -> Result<Value> {
    let namespace = format!("source::{}", registration.id());
    let mut schema = registration_schema(registration.schema(), &namespace, definitions)?;
    let object = schema
        .as_object_mut()
        .context("registered Source schema root must be an object")?;
    object
        .entry("type")
        .or_insert_with(|| Value::String("object".into()));
    object
        .entry("properties")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .context("registered Source schema properties must be an object")?
        .insert(
            "kind".into(),
            json!({ "type": "string", "enum": [registration.id()] }),
        );
    require_property(object, "kind")?;
    Ok(schema)
}

/// Wraps one registered Action input schema under the existing `with` TOML field.
fn action_variant(
    registration: &ActionRegistration,
    definitions: &mut BTreeMap<String, Value>,
) -> Result<Value> {
    let namespace = format!("action::{}", registration.id());
    let inputs = registration_schema(registration.schema(), &namespace, definitions)?;
    let requires_inputs = inputs
        .get("required")
        .and_then(Value::as_array)
        .is_some_and(|required| !required.is_empty());
    let required = if requires_inputs {
        json!(["uses", "with"])
    } else {
        json!(["uses"])
    };

    Ok(json!({
        "type": "object",
        "additionalProperties": false,
        "required": required,
        "properties": {
            "id": {
                "type": "string",
                "pattern": "^[A-Za-z_][A-Za-z0-9_]*$"
            },
            "uses": { "type": "string", "enum": [registration.id()] },
            "with": inputs
        }
    }))
}

/// Extracts and namespaces a registration's local definitions before inlining its root.
fn registration_schema(
    schema: &schemars::schema::RootSchema,
    namespace: &str,
    definitions: &mut BTreeMap<String, Value>,
) -> Result<Value> {
    let mut root = serde_json::to_value(schema)?;
    let object = root
        .as_object_mut()
        .context("registered schema root must be an object")?;
    object.remove("$schema");
    object.remove("title");

    if let Some(local_definitions) = object.remove("definitions") {
        for (name, mut definition) in local_definitions
            .as_object()
            .context("registered schema definitions must be an object")?
            .clone()
        {
            rewrite_refs(&mut definition, namespace);
            definitions.insert(format!("{namespace}::{name}"), definition);
        }
    }
    rewrite_refs(&mut root, namespace);
    Ok(root)
}

/// Inserts a discriminator into `required` without disturbing registration field order.
fn require_property(object: &mut Map<String, Value>, property: &str) -> Result<()> {
    let required = object
        .entry("required")
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .context("registered schema required field must be an array")?;
    if !required.iter().any(|value| value == property) {
        required.insert(0, Value::String(property.into()));
    }
    Ok(())
}

/// Rewrites local `#/definitions/...` references to their registration namespace.
fn rewrite_refs(value: &mut Value, namespace: &str) {
    match value {
        Value::Object(object) => {
            if let Some(reference) = object
                .get("$ref")
                .and_then(Value::as_str)
                .map(str::to_owned)
            {
                if let Some(name) = reference.strip_prefix("#/definitions/") {
                    object.insert(
                        "$ref".into(),
                        Value::String(format!(
                            "#/definitions/{}",
                            pointer_segment(&format!("{namespace}::{name}"))
                        )),
                    );
                }
            }
            for child in object.values_mut() {
                rewrite_refs(child, namespace);
            }
        }
        Value::Array(values) => {
            for child in values {
                rewrite_refs(child, namespace);
            }
        }
        _ => {}
    }
}

/// Escapes a JSON Pointer segment so Action IDs containing `/` remain valid references.
fn pointer_segment(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Locks deterministic discriminator order to Registry order, not insertion order.
    #[test]
    fn schema_lists_every_builtin_variant_in_deterministic_order() {
        let registry = crate::cli::register_builtins().unwrap();
        let schema = workspace_schema(&registry).unwrap();

        let source_ids = schema["properties"]["sources"]["items"]["properties"]["source"]["oneOf"]
            .as_array()
            .unwrap()
            .iter()
            .map(|variant| variant["properties"]["kind"]["enum"][0].as_str().unwrap())
            .collect::<Vec<_>>();
        let action_ids = schema["properties"]["sources"]["items"]["properties"]["actions"]["items"]
            ["oneOf"]
            .as_array()
            .unwrap()
            .iter()
            .map(|variant| variant["properties"]["uses"]["enum"][0].as_str().unwrap())
            .collect::<Vec<_>>();

        assert_eq!(source_ids, ["github", "jira", "qmd"]);
        assert_eq!(action_ids, ["agentboard/run-cmd", "agentboard/worktree"]);
        assert_eq!(
            schema["properties"]["sources"]["items"]["properties"]["actions"]["items"]["oneOf"][0]
                ["properties"]["id"]["pattern"],
            "^[A-Za-z_][A-Za-z0-9_]*$"
        );
    }
}

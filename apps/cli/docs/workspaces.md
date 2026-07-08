---
title: Workspaces
---

# Workspaces

A Workspace is a TOML file that names Sources and the Actions to run for each Source.

## Location

Named workspaces live under the user config directory:

```text
~/.config/agentboard/work.toml
```

Run by name:

```bash
agentboard run work
```

Or pass a path:

```bash
agentboard run ./work.toml
```

## Workspace ids

AgentBoard uses the workspace id in Store paths and action environment variables.

- Named workspace: `work`
- Path workspace: file stem plus canonical path hash, for example `work-a1b2c3d4e5f6`

## Minimal shape

```toml
[[sources]]
id = "local"

[sources.source]
kind = "qmd"
collections = ["tasks"]
query = "intent: ready work"

[[sources.actions]]
uses = "agentboard/run-cmd"

[sources.actions.with]
cmd = "echo {{ item.id }}"
```

Unknown fields are validation errors, except arbitrary keys under `[sources.actions.with]`.

## Where specific config lives

Source-specific fields are documented by source crates:

- [QMD source](/sources/qmd)
- [Jira source](/sources/jira)

Action-specific inputs are documented by action crates:

- [`agentboard/create-worktree`](/actions/worktree)
- [`agentboard/run-cmd`](/actions/run-cmd)

## CLI validation rules

- Source ids must be non-empty and unique.
- Unknown Actions fail validation.
- Unknown Workspace fields fail validation, except arbitrary keys under `[sources.actions.with]`.

Generate the JSON Schema from the same typed model:

```bash
agentboard schema > agentboard.schema.json
```

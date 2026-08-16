---
title: Workspaces
---

# Workspaces

A Workspace is a TOML file that names Sources and the Actions to run for each Source.

## Location

When the Workspace argument is omitted, AgentBoard reads `.agentboard.toml` from the current directory:

```bash
agentboard run
```

AgentBoard checks only the current directory. It does not search parent directories.

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

An explicit name or path always takes precedence over `.agentboard.toml`.

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

Unknown fields are validation errors. Keys under `[sources.actions.with]` must match the selected Action registration.

## Where specific config lives

Source-specific fields are documented by source crates:

- [QMD source](/sources/qmd)
- [Jira source](/sources/jira)

Action-specific inputs are documented by action crates:

- [`agentboard/worktree`](/actions/worktree)
- [`agentboard/run-cmd`](/actions/run-cmd)

## CLI validation rules

- Source ids must be non-empty and unique.
- Action ids are optional, unique within one Source, and match `[A-Za-z_][A-Za-z0-9_]*`.
- Unknown Actions fail validation.
- Unknown Workspace, Source, and typed Action input fields fail validation.

Generate the JSON Schema from the same registered Source and Action schemas used by loading:

```bash
agentboard schema > agentboard.schema.json
```

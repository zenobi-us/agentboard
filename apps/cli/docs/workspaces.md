---
title: Workspaces
---

# Workspaces

A Workspace is one TOML file. It names Sources and the Actions for each Source.

## File selection

Operational commands use `.agentboard.toml` in the current directory when no Workspace argument is provided:

```bash
agentboard run
```

AgentBoard does not search parent directories. A supplied path selects exactly one file. AgentBoard does not merge files, load profiles, or apply field-level CLI overrides.

Named Workspaces are not resolved by operational commands. `workspace list` only lists `*.toml` files under the platform config directory. Pass the listed file path to another command:

```bash
agentboard workspace list
agentboard run "$HOME/.config/agentboard/work.toml"
```

The config directory follows the platform. On Linux, `XDG_CONFIG_HOME` sets the base directory; otherwise AgentBoard uses `~/.config`. AgentBoard adds the `agentboard` directory below that base.

The Workspace id uses the canonical path, so moving a Workspace creates a different Store location.

## Minimal shape

```toml
[[sources]]
id = "local"

[sources.source]
uses = "@agentboard/source-qmd"
collections = ["tasks"]
query = "intent: ready work"

[[sources.actions]]
uses = "@agentboard/action-run-cmd"

[sources.actions.with]
cmd = "echo {{ item.reference_id }}"
```

A Source id must be a non-empty unique string. A Source can omit `actions`; this means that no Action runs and the Store reports the Action plan as successful.

Each Source package owns its fields. Each Action package owns the fields under `[sources.actions.with]`. Unknown fields fail Workspace validation. Action input values are strings in data Workspaces.

## Validation

Validation has two layers:

- Workspace loading checks TOML structure, required fields, types, plugin names, unknown fields, Source ids, and Action ids.
- `doctor` and the normal Run path perform package-specific validation and environment checks.

Action ids are optional. When present, an Action id must match `[A-Za-z_][A-Za-z0-9_]*` and be unique within its Source. Later named Actions can use the id in templates.

`agentboard schema` describes structural fields and types. It does not express every runtime rule. Use `doctor` to test the configured environment.

## Source and Action docs

- [QMD source](/sources/qmd)
- [Jira source](/sources/jira)
- [GitHub source](/sources/github)
- [`@agentboard/action-worktree`](/actions/worktree)
- [`@agentboard/action-run-cmd`](/actions/run-cmd)

# AgentBoard

AgentBoard is a Rust CLI for turning task-tracking sources into local agent work queues, then running source-configured actions for matching items.

Think:

```text
Run workspace -> read sources -> update local store -> run pending actions
```

## Vision

AgentBoard is a small automation layer for agent-driven work queues. It should not become another project tracker. The source of truth stays in Jira, Linear, GitHub, or local markdown. AgentBoard keeps a local copy so agents and scripts can work consistently, offline-ish, and with repeatable rules.

## MVP scope

The first useful slice is deliberately narrow:

1. Load TOML workspace config by name or explicit path.
2. Collect markdown-backed items through QMD collections.
3. Store normalized item observations and action attempts in per-source JSONL files.
4. Render MiniJinja templates for action inputs.
5. Run pending `agentboard/create-worktree` and `agentboard/run-cmd` actions.
6. Inspect stored state with `list` and `show`.
7. Validate environment/config with `doctor`.
8. Print workspace JSON Schema with `schema`.

GitHub, Jira, Linear, YAML/JSON config files, and user-defined actions can follow after the local loop works.

## Workspace config

Workspaces live in user config or at an explicit path:

```text
~/.config/agentboard/
  work.toml
  personal.toml
```

`agentboard run work` loads `~/.config/agentboard/work.toml`.
`agentboard run ./work.toml` loads that file directly.

MVP config is TOML only. The schema is generated from the same typed config model:

```bash
agentboard schema > agentboard.schema.json
```

### Example workspace

```toml
[[sources]]
id = "local"

[sources.source]
kind = "qmd"
collections = ["tasks"]
query = "intent: Find ready or high-priority work items\nlex: status ready priority high"
limit = 50

[[sources.actions]]
uses = "agentboard/create-worktree"

[sources.actions.with]
repo = "~/Projects/MyProject"
root = "~/Projects/MyProject.worktrees/{{ item.id }}"
branch = "{{ item.id }}"

[[sources.actions]]
uses = "agentboard/run-cmd"

[sources.actions.with]
cmd = "zellij action new-tab --name {{ item.id }}"
cwd = "~/Projects/MyProject"
```

Unknown workspace/source/action fields are validation errors, except arbitrary keys under an action `with` table.

## QMD source

MVP supports only `kind = "qmd"`. QMD is an optional runtime dependency: AgentBoard builds without it, but `run` and `doctor` fail clearly for QMD sources when the `qmd` command is missing.

A QMD source passes its query through to `qmd query` against named collections. Each retrieved markdown document becomes one item and should have YAML frontmatter with at least:

```markdown
---
id: AB-001
title: Create the first worktree
status: ready
priority: high
labels:
  - agent
---

Task details live here.
```

Normalized item fields:

- `id` — from frontmatter `id` by default
- `title` — from frontmatter `title` by default
- `status` — from frontmatter `status` by default
- `url` — frontmatter `url`, or the QMD document reference when missing
- `source_id` — workspace source id
- `source_kind` — `qmd`
- `raw` — structured object containing QMD result metadata, frontmatter, and markdown body

Optional field mapping lives under `[sources.source.map]` and uses frontmatter keys, including dotted paths:

```toml
[sources.source.map]
id = "agentboard.id"
title = "name"
status = "state"
url = "links.html"
```

Duplicate item ids within one source are source errors.

QMD sources are read-only. AgentBoard never edits source markdown files in the MVP.

## Queries

Queries are Source-owned. For QMD, `query` is required inside `[sources.source]` and is passed to `qmd query`.

Example:

```text
intent: Find work ready for agents
lex: status ready labels agent
vec: actionable tasks ready for autonomous coding agents
```

Each QMD source must name at least one collection. AgentBoard does not search the whole local QMD index by default.

## Store

AgentBoard stores data under the user's XDG data directory:

```text
${XDG_DATA_HOME:-~/.local/share}/agentboard/<workspace-id>/
  sources/
    <source-id>/
      items.jsonl
      actions.jsonl
```

Workspace ids are stable:

- Named workspace: `work`
- Explicit path: filename stem plus short hash of the canonical path, e.g. `work-a1b2c3d4e5f6`

`items.jsonl` appends item observations. `actions.jsonl` appends action attempts. `list` and `show` derive latest state from these files.

## Actions

Actions belong to the source that declares them. There are no global workspace actions in the MVP.

Built-in actions:

- `agentboard/create-worktree` — create or reuse a git worktree for an item.
- `agentboard/run-cmd` — run a shell command rendered from item/workspace/source/action context.

`agentboard/*` is reserved for built-in actions. Unknown actions are validation errors.

An action runs when no previous successful action result exists for:

```text
(source_id, item.id, source_action_index, rendered_action_hash)
```

Failed actions retry on the next `run` or `watch` until they succeed.

Actions run in source config order, then item id order, then action config order. Actions are ordered and blocking per item: if action 1 fails, action 2 for that item does not run yet.

### `agentboard/create-worktree`

Uses plain `git worktree`.

Required inputs:

- `repo`
- `root`
- `branch`

Behavior:

- If `root` already exists and is the intended worktree for `branch`, success.
- If `root` exists but does not match the intended worktree/branch, fail that item action.
- If `branch` already exists, add a worktree for the existing branch.

### `agentboard/run-cmd`

Uses the platform shell (`sh -c` on Unix) with no interactive stdin.

Required inputs:

- `cmd`

Optional inputs:

- `cwd` — defaults to the AgentBoard process cwd.

The command inherits the current environment plus `AGENTBOARD_WORKSPACE_ID`, `AGENTBOARD_SOURCE_ID`, and `AGENTBOARD_ITEM_ID`.

Stdout and stderr are captured in the action result and capped at 64 KiB each.

Workspace configs are trusted local code, like a Makefile or package script. AgentBoard does not sandbox commands.

## Templates

Action inputs are MiniJinja templates. Context includes:

- `workspace`
- `source`
- `item`
- `action`

MVP registers a custom `slugify` filter for branch/path-safe strings.

Configured paths expand leading `~` and environment variables after template rendering.

## Execution model

```text
[load workspace]
      |
      v
[validate config]
      |
      v
[acquire workspace lock]
      |
      v
[run source pipelines concurrently]
      |
      v
[each source: collect -> store item observations -> run serial pending actions]
      |
      v
[append action attempts]
```

Failures are source/item scoped where possible. Other sources continue. The process exits nonzero if any source or action failed.

A workspace lock prevents overlapping `run` or `watch` processes for the same workspace. `watch` holds the lock until it exits.

## CLI shape

```text
agentboard run <workspace> [--dry-run]
agentboard watch <workspace> [--interval 60s]
agentboard list <workspace> [--json]
agentboard show <workspace> <item-id> [--json]
agentboard doctor <workspace>
agentboard schema
```

- `run` executes the full pipeline once.
- `run --dry-run` parses, collects, renders, and prints pending actions without writing store files or executing actions.
- `watch` repeatedly runs the pipeline on an interval and exits cleanly on Ctrl-C.
- `list` shows latest item status plus derived action state.
- `show` prints one normalized item plus latest action results.
- `doctor` validates one workspace, store writability, and required external commands.
- `schema` prints the workspace JSON Schema to stdout.

## Non-goals

- No hosted service.
- No UI in first pass.
- No tracker replacement.
- No public `collect` command in the MVP; collect is an internal stage of `run`.
- No user-defined action registry until built-in actions prove the shape.
- No YAML/JSON workspace config until TOML is useful.
- No GitHub/Jira/Linear source adapters until QMD-backed markdown works end to end.
- No full Jira/Linear field model before real use demands it.

## Implementation notes

Use a thin `main.rs` plus library modules by concern: CLI, config, model, sources, actions, store, and templates.

The MVP source abstraction should be an async `SourceAdapter` trait with a `collect` operation. Leave short TODO comments for future validation/auth/pagination hooks; do not build a plugin registry yet.

The MVP action abstraction should be a tiny trait for built-ins only. User actions are future work.

Tests should include Rust unit tests for config/source/store/template logic and a separate Bats task for CLI integration behavior.

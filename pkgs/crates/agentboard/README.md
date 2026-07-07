# AgentBoard

AgentBoard is a Rust CLI for collecting task-tracking items from multiple sources into a local workspace store, then running configured steps against those items.

Think:

```text
Collect -> Store locally -> Run workspace actions
```

## Vision

AgentBoard is a small automation layer for agent-driven work queues. It should not become another project tracker. The source of truth stays in Jira, Linear, GitHub, or local markdown. AgentBoard keeps a local copy so agents and scripts can work consistently, offline-ish, and with repeatable rules.

## Workspace-driven config

Workspaces live in user config:

```text
~/.config/agentboard/
  workspace-one.toml
  workspace-two.toml
  personal.yaml
```

Each workspace declares sources, queries, and actions:

```toml
[[sources]]
id = "foo"
query = "status:ready"

[sources.source]
kind = "jira"
url = "https://example.atlassian.net"
credential_helper = "op read op://vault/jira/token"

[[sources.actions]]
uses = "agentboard/sync"

[[sources.actions]]
uses = "agentboard/create-worktree"

[sources.actions.with]
repo = "~/Projects/MyProject"
root = "{{ repo }}.worktrees/{{ branchname }}"
branch = "{{ item.id }}/{{ item.title | slugify }}"

[[sources.actions]]
uses = "agentboard/run-cmd"

[sources.actions.with]
cmd = "zellij action new-tab --name {{ item.id }}"
```

YAML should express the same model for humans who prefer it.

## Core concepts

### Source

A source fetches task-like items from an external or local system.

Planned source kinds:

- `github-issues`
- `github-projects`
- `jira`
- `linear`
- `markdown`

### Item

A normalized local task record. Minimum shape:

- `id`
- `title`
- `url`
- `source_id`
- `source_kind`
- `status`
- `raw`

`raw` keeps the original payload so AgentBoard does not need to model every tracker field up front.

### Store

A local per-workspace cache of collected items and action results.

Goal: boring files first, likely JSONL or SQLite only when file storage becomes painful. Do not invent a sync database early.

### Action

An action runs after collection. Built-ins:

- `agentboard/sync` — persist fetched item state locally.
- `agentboard/create-worktree` — create or reuse a git worktree for an item.
- `agentboard/run-cmd` — run a command rendered from item/workspace context.

Action inputs are MiniJinja templates. AgentBoard provides helpers like `slugify` for branch names.

## Execution model

```text
[load workspace]
      |
      v
[validate config]
      |
      v
[collect sources]
      |
      v
[write local store]
      |
      v
[run actions per item]
      |
      v
[record results]
```

Failures should be item-scoped where possible. One broken Jira item should not kill unrelated GitHub items unless config says fail-fast.

## CLI shape

Likely commands:

```text
agentboard list
agentboard collect <workspace>
agentboard run <workspace>
agentboard show <workspace> <item-id>
agentboard doctor
```

Keep CLI boring. Add subcommands only when they have a real workflow.

## Non-goals

- No hosted service.
- No UI in first pass.
- No tracker replacement.
- No plugin system until built-in actions prove too small.
- No full Jira/Linear field model before real use demands it.

## First useful slice

1. Load TOML workspace config.
2. Collect local markdown items.
3. Store normalized items locally.
4. Render MiniJinja templates for action inputs.
5. Run `create-worktree` and `run-cmd`.

GitHub/Jira/Linear can follow after the local loop works.

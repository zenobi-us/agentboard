# AgentBoard

Rust CLI for collecting task-tracking items from many sources into a local store, then running workspace-configured actions.

## Purpose

AgentBoard is an automation bridge for agent work queues. It queries sources such as QMD-backed markdown, copies items locally, then runs rules from a TOML workspace file.

```text
Query sources -> Store locally -> Run actions
```

## Workspace config

Workspace files live in user config:

```text
~/.config/agentboard/
  work.toml
  personal.toml
```

Example:

```toml
[[sources]]
id = "foo"

[sources.source]
kind = "qmd"
collections = ["tasks"]
query = "intent: Find ready agent work\nlex: status ready"

[[sources.actions]]
uses = "agentboard/create-worktree"

[sources.actions.with]
repo = "~/Projects/MyProject"
root = "~/Projects/MyProject.worktrees/{{ item.id }}"
branch = "{{ item.id }}/{{ item.title | slugify }}"

[[sources.actions]]
uses = "agentboard/run-cmd"

[sources.actions.with]
cmd = "zellij action new-tab --name {{ item.id }}"
```

Jira Cloud source example:

```toml
[[sources]]
id = "jira"

[sources.source]
kind = "jira"
site = "https://your-domain.atlassian.net"
email_env = "JIRA_EMAIL"
token_env = "JIRA_API_TOKEN"
jql = "project = AB AND statusCategory != Done ORDER BY updated DESC"
limit = 50
fields = ["customfield_10010"]

[sources.source.map]
id = "key"
title = "fields.summary"
status = "fields.status.name"
```

## Projects

- `apps/cli` — Rust CLI crate.
- `pkgs/crates/agentboard-core` — shared model/types.
- `pkgs/crates/agentboard-source-*` — one crate per source adapter.
- `pkgs/crates/agentboard-action-*` — one crate per action executor.
- `apps/docs` — docs app.
- `pkgs/tools/deployment` — release/deploy helpers.

## Setup

```bash
proto install
bun install
moon query projects
```

## Common tasks

```bash
moon run agentboard:build
moon run agentboard:test
moon run docs:dev
```

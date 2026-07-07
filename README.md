# AgentBoard

Rust CLI for collecting task-tracking items from many sources into a local store, then running workspace-configured actions.

## Purpose

AgentBoard is an automation bridge for agent work queues. It collects items from Jira, Linear, local markdown, GitHub Projects, and GitHub Issues, copies them locally, then runs rules from a TOML/YAML workspace file.

```text
Collect -> Store locally -> Run actions
```

## Workspace config

Workspace files live in user config:

```text
~/.config/agentboard/
  work.toml
  personal.yaml
```

Example:

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

## Projects

- `pkgs/crates/agentboard` — Rust CLI crate.
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

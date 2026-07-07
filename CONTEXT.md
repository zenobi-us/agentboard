# Context

AgentBoard is a Moon + proto monorepo for a Rust CLI that collects task-tracking items into a local store and runs configured actions against them.

## Components

- Rust CLI loads workspace files from `~/.config/agentboard/*.toml` and `*.yaml`.
- Sources fetch task-like items from Jira, Linear, local markdown, GitHub Projects, and GitHub Issues.
- Store keeps normalized item records plus raw source payloads.
- Actions run after collection, including sync, git worktree creation, and templated commands.
- Docs app explains setup, workspace config, source kinds, and actions.

## Important constraints

- Config is workspace-driven, not repo-driven.
- MiniJinja renders action inputs such as branch names, worktree roots, and commands.
- Source adapters must not know action execution details.
- Action execution must record results in the local store.
- External credentials should come from credential helpers, not stored config secrets.

## Glossary

- Workspace: one config file under `~/.config/agentboard/`.
- Source: a task provider plus query.
- Item: normalized local copy of a task-like record.
- Action: a built-in step run for each collected item.
- Store: local cache of items and action results.

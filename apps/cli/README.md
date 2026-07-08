# AgentBoard CLI

AgentBoard turns task-tracking sources into local agent work queues, then runs source-configured actions for matching items.

```text
workspace -> sources -> store -> templates -> actions
```

The tracker or markdown collection stays the source of truth. AgentBoard keeps an append-only local Store so Runs are inspectable and retryable.

## Quick start

Create a workspace. Source and Action fields are documented in their crate docs; the CLI wires them together:

```toml
[[sources]]
id = "local"

[sources.source]
kind = "qmd"
# qmd fields go here

[[sources.actions]]
uses = "agentboard/run-cmd"

[sources.actions.with]
# run-cmd inputs go here
```

Run it:

```bash
agentboard run ./work.toml --dry-run
agentboard run ./work.toml
agentboard list ./work.toml
```

## Commands

```text
agentboard run <workspace> [--dry-run]
agentboard watch <workspace> [--interval 60s]
agentboard list <workspace> [--json]
agentboard show <workspace> <item-id> [--json]
agentboard doctor <workspace>
agentboard schema
```

See [docs/commands.md](docs/commands.md) for examples.

## Docs

- [Workspaces](docs/workspaces.md) — config files, ids, validation, examples.
- [Sources](docs/sources.md) — how Sources fit into Workspaces, with links to source crate docs.
- [Templates](docs/templates.md) — MiniJinja context, `slugify`, path expansion.
- [Actions](docs/actions.md) — orchestration, retry behavior, with links to action crate docs.
- [Store](docs/store.md) — append-only JSONL layout and derived state.
- [Commands](docs/commands.md) — command reference.
- [Troubleshooting](docs/troubleshooting.md) — common failures.

## Non-goals

- No hosted service.
- No UI.
- No tracker replacement.
- No sandbox for actions; workspace configs are trusted local code.

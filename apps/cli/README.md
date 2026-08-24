# AgentBoard CLI

AgentBoard collects Items from configured Sources, stores local observations, and runs Actions for matching Items.

```text
Workspace -> Sources -> Store -> templates -> Actions
```

The upstream system remains the source of truth. AgentBoard does not include an agent runtime or a hosted service.

## Quick start

Create a TOML Workspace at an explicit path:

```bash
agentboard workspace init ./work.toml
```

Add a Source and an Action:

```toml
[[sources]]
id = "local"

[sources.source]
uses = "@agentboard/source-qmd"
collections = ["tasks"]
query = "intent: ready work items"

[[sources.actions]]
uses = "@agentboard/action-run-cmd"

[sources.actions.with]
cmd = "echo {{ item.reference_id }}"
```

Run the Workspace:

```bash
agentboard doctor ./work.toml
agentboard run ./work.toml --dry-run
agentboard run ./work.toml
agentboard list ./work.toml
```

## Commands

```text
agentboard init <path>
agentboard workspace init <path>
agentboard workspace list
agentboard workspace edit <path>
agentboard workspaces
agentboard run [workspace] [--dry-run] [--watch] [--interval 60s]
agentboard list [workspace] [--json] [--watch] [--interval 60s]
agentboard show [workspace] <item> [--json] [--watch] [--interval 60s]
agentboard dashboard [workspace]
agentboard tui
agentboard doctor [workspace]
agentboard schema
```

Operational commands use `.agentboard.toml` when the Workspace argument is omitted. They do not search parent directories. A Workspace path is one TOML file; AgentBoard does not merge files or apply field overrides.

## Docs

- [Workspaces](docs/workspaces.md) — file selection, validation, and examples.
- [Sources](docs/sources.md) — Source execution and normalized Items.
- [Templates](docs/templates.md) — MiniJinja context and path expansion.
- [Actions](docs/actions.md) — ordering, retry behavior, and trust model.
- [Store](docs/store.md) — append-only JSONL records and derived state.
- [Commands](docs/commands.md) — command reference.
- [Troubleshooting](docs/troubleshooting.md) — common errors.

The `tui` command is an experimental OpenTUI shell. It is not the Store-backed `dashboard` command and does not provide a supported data view.

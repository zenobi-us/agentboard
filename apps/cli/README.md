# ClankPipe CLI

ClankPipe collects Items from configured Sources, stores local observations, and runs Actions for matching Items.

```text
Workspace -> Sources -> Store -> templates -> Actions
```

The upstream system remains the source of truth. ClankPipe does not include an agent runtime or a hosted service.

## Quick start

Create a TOML Workspace at an explicit path:

```bash
clankpipe workspace init ./work.toml
```

Add a Source and an Action:

```toml
[[sources]]
id = "local"

[sources.source]
uses = "@clankpipe/source-qmd"
collections = ["tasks"]
query = "intent: ready work items"

[[sources.actions]]
uses = "@clankpipe/action-run-cmd"

[sources.actions.with]
cmd = "echo {{ item.reference_id }}"
```

Run the Workspace:

```bash
clankpipe doctor ./work.toml
clankpipe run ./work.toml --dry-run
clankpipe run ./work.toml
clankpipe list ./work.toml
```

## Commands

```text
clankpipe init <path>
clankpipe workspace init <path>
clankpipe workspace list
clankpipe workspace edit <path>
clankpipe workspaces
clankpipe run [workspace] [--dry-run] [--watch] [--interval 60s]
clankpipe list [workspace] [--json] [--watch] [--interval 60s]
clankpipe show [workspace] <item> [--json] [--watch] [--interval 60s]
clankpipe tui [workspace]
clankpipe doctor [workspace]
clankpipe schema
```

Operational commands use `.clankpipe.toml` when the Workspace argument is omitted. They do not search parent directories. A Workspace path is one TOML file; ClankPipe does not merge files or apply field overrides.

## Docs

- [Workspaces](docs/workspaces.md) — file selection, validation, and examples.
- [Sources](docs/sources.md) — Source execution and normalized Items.
- [Templates](docs/templates.md) — MiniJinja context and path expansion.
- [Actions](docs/actions.md) — ordering, retry behavior, and trust model.
- [Store](docs/store.md) — append-only JSONL records and derived state.
- [Commands](docs/commands.md) — command reference.
- [Troubleshooting](docs/troubleshooting.md) — common errors.

The `tui` command is the interactive Store-backed terminal view. It shows one focused Workspace tree and opens Workspace, Source, Action, and Item details in drawers.

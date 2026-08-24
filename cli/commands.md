# Commands (/cli/commands)



# Commands [#commands]

Operational commands accept a Workspace path. If omitted, they use `.agentboard.toml` in the current directory. AgentBoard does not search parent directories.

## `workspace` [#workspace]

Create an empty Workspace. The command creates the path you provide and does not overwrite an existing file:

```bash
agentboard workspace init ./work.toml
```

List TOML files in the platform config directory under `agentboard`. On Linux, this is normally `~/.config/agentboard`. The output is a JSON array of file stems:

```bash
agentboard workspace list
```

Open a Workspace in `$EDITOR`:

```bash
EDITOR=vi agentboard workspace edit ./work.toml
```

`EDITOR` must contain an executable command. AgentBoard passes the absolute Workspace path as its only argument.

`agentboard workspaces` remains an alias for `agentboard workspace list`.

## `run` [#run]

Collect Items, commit a Source Snapshot, render Actions, and execute pending Actions:

```bash
agentboard run ./work.toml
```

Dry run collects Items and renders Actions. It does not acquire the Workspace lock, write Store files, or execute Actions:

```bash
agentboard run ./work.toml --dry-run
```

Watch Mode repeats normal Runs until Ctrl-C. The default interval is 60 seconds. A watched dry run performs one dry Run and then exits.

```bash
agentboard run ./work.toml --watch --interval 30s
```

`--interval` accepts seconds with an optional `s`, such as `30` or `30s`.

Use `--json` or `--output-format json` for structured Run output. Use global `--quiet`, `--verbose`, `--color`, or `--log-file` flags for output control.

## `list` [#list]

List Items from the latest committed Snapshot for each configured Source:

```bash
agentboard list ./work.toml
agentboard list ./work.toml --json
```

JSON output contains `source_id`, `snapshot`, `collection_status`, and `items`. Each Item result is `success`, `error`, or `pending`. A missing Snapshot is different from a committed empty Snapshot.

`list --watch` refreshes the Store view in a terminal. It cannot use `--json` or redirected stdout.

## `show` [#show]

Show one stored Item and its Action attempts:

```bash
agentboard show ./work.toml AB-001
agentboard show AB-001
agentboard show ./work.toml AB-001 --json
```

The short form uses `.agentboard.toml`. The command matches either `item.id` or `item.reference_id`. `show --watch` refreshes the human view and cannot use `--json` or redirected stdout.

## `dashboard` [#dashboard]

Open the read-only Store dashboard:

```bash
agentboard dashboard ./work.toml
```

The Dashboard requires interactive stdin and stdout. It reads committed Snapshots and collection status. It does not collect Sources, execute Actions, acquire the Workspace lock, or write Store files. It refreshes every 60 seconds. Press `r` to refresh or `q` or Esc to exit.

## `tui` [#tui]

Open the experimental OpenTUI shell:

```bash
agentboard tui
```

This shell requires a terminal. It is not a supported Store-backed view.

## `doctor` [#doctor]

Validate a Workspace and its local environment:

```bash
agentboard doctor ./work.toml
```

The command checks Source configuration, Action configuration, and package-specific health requirements. It prints JSON and exits with status `1` when a check reports an error.

## `schema` [#schema]

Print the JSON Schema built from the registered Source and Action packages:

```bash
agentboard schema > agentboard.schema.json
```

The schema describes structure and types. It does not express every semantic rule, such as unique Source ids, non-empty Source maps, or required built-in Action inputs.

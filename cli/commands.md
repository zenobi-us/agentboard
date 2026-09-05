# Commands (/cli/commands)



# Commands [#commands]

Operational commands accept a Workspace path. If omitted, they use `.clankpipe.toml` in the current directory. ClankPipe does not search parent directories.

## `workspace` [#workspace]

Create an empty Workspace. The command creates the path you provide and does not overwrite an existing file:

```bash
clankpipe workspace init ./work.toml
```

List TOML files in the platform config directory under `clankpipe`. On Linux, this is normally `~/.config/clankpipe`. The output is a JSON array of file stems:

```bash
clankpipe workspace list
```

Open a Workspace in `$EDITOR`:

```bash
EDITOR=vi clankpipe workspace edit ./work.toml
```

`EDITOR` must contain an executable command. ClankPipe passes the absolute Workspace path as its only argument.

`clankpipe workspaces` remains an alias for `clankpipe workspace list`.

## `run` [#run]

Collect Items, commit a Source Snapshot, render Actions, and execute pending Actions:

```bash
clankpipe run ./work.toml
```

Dry run collects Items and renders Actions. It does not acquire the Workspace lock, write Store files, or execute Actions:

```bash
clankpipe run ./work.toml --dry-run
```

Watch Mode repeats normal Runs until Ctrl-C. The default interval is 60 seconds. A watched dry run performs one dry Run and then exits.

```bash
clankpipe run ./work.toml --watch --interval 30s
```

`--interval` accepts seconds with an optional `s`, such as `30` or `30s`.

Use `--json` or `--output-format json` for structured Run output. Use global `--quiet`, `--verbose`, `--color`, or `--log-file` flags for output control.

## `list` [#list]

List Items from the latest committed Snapshot for each configured Source:

```bash
clankpipe list ./work.toml
clankpipe list ./work.toml --json
```

JSON output contains `source_id`, `snapshot`, `collection_status`, and `items`. Each Item result is `success`, `error`, or `pending`. A missing Snapshot is different from a committed empty Snapshot.

`list --watch` refreshes the Store view in a terminal. It cannot use `--json` or redirected stdout.

## `show` [#show]

Show one stored Item and its Action attempts:

```bash
clankpipe show ./work.toml AB-001
clankpipe show AB-001
clankpipe show ./work.toml AB-001 --json
```

The short form uses `.clankpipe.toml`. The command matches either `item.id` or `item.reference_id`. `show --watch` refreshes the human view and cannot use `--json` or redirected stdout.

## `tui` [#tui]

Open the experimental OpenTUI shell:

```bash
clankpipe tui ./work.toml
```

The TUI requires a terminal. It shows one focused Workspace tree. Source, Action, and Item details open in drawers.

## `doctor` [#doctor]

Validate a Workspace and its local environment:

```bash
clankpipe doctor ./work.toml
```

The command checks Source configuration, Action configuration, and package-specific health requirements. It prints JSON and exits with status `1` when a check reports an error.

## `schema` [#schema]

Print the JSON Schema built from the registered Source and Action packages:

```bash
clankpipe schema > clankpipe.schema.json
```

The schema describes structure and types. It does not express every semantic rule, such as unique Source ids, non-empty Source maps, or required built-in Action inputs.

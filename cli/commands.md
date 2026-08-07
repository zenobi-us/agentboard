# Commands (/cli/commands)



# Commands [#commands]

## `workspace` [#workspace]

List named Workspaces from `~/.config/agentboard` (or the platform config directory):

```bash
agentboard workspace list
```

Output contains one Workspace name per line, sorted alphabetically. `agentboard workspaces` remains available as a compatibility alias.

Create an empty named Workspace without overwriting an existing config:

```bash
agentboard workspace init work
```

Open an existing named Workspace in the command configured by `$EDITOR`:

```bash
EDITOR="code --wait" agentboard workspace edit work
```

AgentBoard appends the Workspace path as the final editor argument and waits for the editor to exit. `$EDITOR` must be set and non-empty. `workspace edit` accepts a Workspace name, not an explicit file path, and does not create missing Workspaces.

## `run` [#run]

Execute one Workspace Run: load config, read Sources, append item observations, render pending Actions, and execute Actions.

```bash
agentboard run
agentboard run work
agentboard run ./work.toml
```

With no Workspace argument, AgentBoard loads `.agentboard.toml` from the current directory. It does not search parent directories. A supplied name or path keeps the existing explicit selection behavior.

Dry run collects and renders pending Actions without writing Store files or executing Actions:

```bash
agentboard run ./work.toml --dry-run
```

Watch Mode repeats the Run until Ctrl-C. The default interval is 60 seconds.
Intervals are seconds with or without a trailing `s`.

```bash
agentboard run work --watch
agentboard run work --watch --interval 30s
agentboard run work --watch --dry-run
```

A non-dry watched Run holds the Workspace lock until it exits. A watched dry
Run does not acquire the lock or write Store files.

## `list` [#list]

List latest stored items and derived Action state.

```bash
agentboard list
agentboard list work
```

Plain output:

```text
AB-001	ready	pending	Create the first worktree
```

JSON output:

```bash
agentboard list work --json
```

## `show` [#show]

Show one latest stored item and its Action attempts.

```bash
agentboard show AB-001
agentboard show work AB-001
agentboard show work AB-001 --json
```

If the same item id exists in multiple Store item buckets, use the qualified form shown in the ambiguity error:

```bash
agentboard show work jira-team-a-atlassian-net-abc123:AB-001
```

## `doctor` [#doctor]

Validate a Workspace and local environment.

```bash
agentboard doctor
agentboard doctor work
```

Checks include:

* Workspace config validation.
* Store directory writability.
* Required Source commands, for example `qmd`.
* Required Action commands, for example `git` for `agentboard/worktree`.
* Source reachability by collecting items.

## `schema` [#schema]

Print the Workspace JSON Schema.

```bash
agentboard schema > agentboard.schema.json
```

Use this for editor validation and config discovery.

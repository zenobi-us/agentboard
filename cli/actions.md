# Actions (/cli/actions)



# Actions [#actions]

Actions are side effects that run for each Item collected from a Source.

Define them in the `sources.actions` array in your workspace config.

Each action is a sequential step that runs for each item in the source. Actions are blocking per item.

This CLI doc only covers orchestration: ordering, retry identity, and trust model. Action-specific config belongs in each action package doc.

## Workspace Shape [#workspace-shape]

```toml
[[sources.actions]]
uses = "@clankpipe/action-run-cmd"
open = "gh issue view {{ item.reference_id }} --web"

[sources.actions.with]
cmd = "echo {{ item.id }}"
```

## Execution Order [#execution-order]

Sources run concurrently. Item Actions inside one Source are serial.

For each Item returned by a Source, AgentBoard executes Actions in the order defined in the `sources.actions` array.

Actions are blocking per item. If action `0` fails for an item, action `1` for that item does not run during that Run.

An Action can define an optional `open` command. The TUI runs this command when you press `O` in the Action Item view. The command uses the same MiniJinja context as Action inputs.

## Retry Identity [#retry-identity]

An Action runs when no previous successful attempt exists for:

```text
(source_id, item.id, source_action_index, rendered_action_hash)
```

Changing the rendered inputs changes the hash and makes the Action eligible to run again.

Failed Actions retry on the next `run`, including `run --watch`, until they succeed.

## Action Docs [#action-docs]

* [`@clankpipe/action-worktree`](/actions/worktree)
* [`@clankpipe/action-run-cmd`](/actions/run-cmd)

## Trust Model [#trust-model]

Workspace configs are trusted local code, like a `Makefile`. AgentBoard does not sandbox commands.

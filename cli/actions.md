# Actions (/cli/actions)



# Actions [#actions]

Actions are steps that are run after new items have been retrieved from a source.

Define them in the `sources.actions` array in your workspace config.

Each action is a sequential step that runs for each item in the source. Actions are blocking per item.

This CLI doc only covers orchestration: ordering, retry identity, and trust model. Action-specific config belongs in each action crate doc.

## Workspace Shape [#workspace-shape]

```toml
[[sources.actions]]
uses = "agentboard/run-cmd"

[sources.actions.with]
cmd = "echo {{ item.id }}"
```

## Execution Order [#execution-order]

Sources run concurrently. Item Actions inside one Source are serial.

For each item retrieved by a Source, agentboard will then execute Actions in the order they are defined in the `sources.actions` array.

Actions are blocking per item. If action `0` fails for an item, action `1` for that item does not run during that Run.

## Retry Identity [#retry-identity]

An Action runs when no previous successful attempt exists for:

```text
(source_id, item.id, source_action_index, rendered_action_hash)
```

Changing the rendered inputs changes the hash and makes the Action eligible to run again.

Failed Actions retry on the next `run` or `watch` until they succeed.

## Action Docs [#action-docs]

* [`agentboard/create-worktree`](/actions/worktree)
* [`agentboard/run-cmd`](/actions/run-cmd)

## Trust Model [#trust-model]

Workspace configs are trusted local code, like a `Makefile`. AgentBoard does not sandbox commands.

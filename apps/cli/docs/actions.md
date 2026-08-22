---
title: Actions
---

# Actions

Actions are steps that are run after new items have been retrieved from a source.

Define them in the `sources.actions` array in your workspace config. 

Each action is a sequential step that runs for each item in the source. Actions are blocking per item.

This CLI doc only covers orchestration: ordering, retry identity, and trust model. Action-specific config belongs in each action package doc.

## Workspace Shape

```toml
[[sources.actions]]
uses = "@agentboard/action-run-cmd"

[sources.actions.with]
cmd = "echo {{ item.id }}"
```

## Execution Order

Sources run concurrently. Item Actions inside one Source are serial.

For each item retrieved by a Source, agentboard will then execute Actions in the order they are defined in the `sources.actions` array.

Actions are blocking per item. If action `0` fails for an item, action `1` for that item does not run during that Run.


## Retry Identity

An Action runs when no previous successful attempt exists for:

```text
(source_id, item.id, source_action_index, rendered_action_hash)
```

Changing the rendered inputs changes the hash and makes the Action eligible to run again.

Failed Actions retry on the next `run`, including `run --watch`, until they succeed.

## Action Docs

- [`@agentboard/action-worktree`](/actions/worktree)
- [`@agentboard/action-run-cmd`](/actions/run-cmd)

## Trust Model

Workspace configs are trusted local code, like a `Makefile`. AgentBoard does not sandbox commands.

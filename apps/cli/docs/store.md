---
title: Store
---

# Store

The Store is AgentBoard's local append-only record of item observations and Action attempts.

It is not the source of truth. The tracker or markdown collection remains the source of truth.

## Location

AgentBoard stores data under the user's XDG data directory:

```text
${XDG_DATA_HOME:-~/.local/share}/agentboard/<workspace-id>/
  run.lock
  sources/
    <source-id>/
      items.jsonl
      actions.jsonl
```

## Workspace lock

`run` and `watch` acquire `run.lock` for the Workspace.

- `run --dry-run` skips the lock and does not write Store files.
- `watch` holds the lock until it exits.
- Overlapping normal Runs for the same Workspace fail.

## `items.jsonl`

Each line is one normalized item observation.

A new Run appends new observations. It does not rewrite older lines.

`list` and `show` derive the latest item by item id from the append-only file.

## `actions.jsonl`

Each line is one Action attempt.

Action attempts include:

- timestamp
- source id
- item id
- source Action index
- Action name
- rendered Action hash
- success flag
- stdout
- stderr
- message

Successful attempts are used to skip completed work on later Runs.

## Derived action state

`list` shows a derived state for each item:

- `pending` — no Action attempt exists for that item.
- `succeeded` — at least one attempt exists and no attempt failed.
- `failed` — at least one attempt failed.

This is display state, not tracker state.

## Inspecting by hand

The Store is plain JSONL. Use normal shell tools:

```bash
tail -n 20 ~/.local/share/agentboard/work/sources/local/actions.jsonl
jq . ~/.local/share/agentboard/work/sources/local/items.jsonl
```

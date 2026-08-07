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
  items-<source.slug>.jsonl
  actions-<source.slug>-<source.hash>.jsonl
```

`source.slug` identifies the upstream item universe. For Jira, it is derived from the normalized site URL because Jira issue keys are only unique inside one Jira organization. Two Jira Sources for the same site and different JQL views share an item file.

`source.hash` identifies the configured Source view and Action plan. Changing JQL, mappings, or Actions creates a different action file without duplicating the broad item Store.

Legacy `sources/<source-id>/items.jsonl` and `sources/<source-id>/actions.jsonl` files are not migrated automatically.

## Workspace lock

`run` and `watch` acquire `run.lock` for the Workspace.

- `run --dry-run` skips the lock and does not write Store files.
- `watch` holds the lock until it exits.
- Overlapping normal Runs for the same Workspace fail.

## `items-<source.slug>.jsonl`

Each line is one normalized item observation.

Each Item contains both identities:

- `id` — adapter-owned identity used for Store and Action matching.
- `reference_id` — provider-facing reference available to templates.

A new Run appends new observations. It does not rewrite older lines.

`list` derives the latest item by item id inside each item universe. JSON output includes `source_slug` for disambiguation. `show` returns one latest matching item, and accepts `source.slug:item-id` when the same item id exists in multiple item universes.

## `actions-<source.slug>-<source.hash>.jsonl`

Each line is one Action attempt.

Action attempts include:

- timestamp
- source id
- item id
- source Action index
- Action name
- rendered Action hash
- outcome (`success`, `failure`, or `cancelled`)
- stdout
- stderr
- message

Successful attempts are used to skip completed work on later Runs.

## Derived action state

`list` shows a derived state for each item:

- `pending` — no Action attempt exists, or the latest attempt was cancelled.
- `succeeded` — the latest attempt for every Action succeeded.
- `failed` — the latest attempt for an Action failed.

Cancelled attempts remain eligible for retry on the next Run.

This is display state, not tracker state.

## Inspecting by hand

The Store is plain JSONL. Use normal shell tools:

```bash
tail -n 20 ~/.local/share/agentboard/work/actions-jira-team-a-atlassian-net-abc123-def456.jsonl
jq . ~/.local/share/agentboard/work/items-jira-team-a-atlassian-net-abc123.jsonl
```

## Store schema migration

`reference_id` is required in Item records. Stores created before this field was
added are intentionally not migrated automatically. Remove the affected
`items-<source.slug>.jsonl` file and run the Workspace again to rebuild it.

Keeping the Action Store preserves its records, but Jira, QMD, and GitHub
Sources with `field_map.id` changed Item identities with this schema. Their old
successful attempts do not suppress Actions for the rebuilt Items. Review or
temporarily disable configured Actions before the rebuilding Run if repeating
them could cause duplicate side effects.

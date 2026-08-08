---
title: Store
---

# Store

The Store is AgentBoard's local append-only record of Item observations, Source
Snapshot boundaries, and Action attempts.

It is not the source of truth. The tracker or markdown collection remains the source of truth.

## Location

AgentBoard stores data under the user's XDG data directory:

```text
${XDG_DATA_HOME:-~/.local/share}/agentboard/<workspace-id>/
  run.lock
  items-<source.slug>.jsonl
  actions-<source.slug>-<source.hash>.jsonl
  items-<source.slug>.snapshots
```

`source.slug` identifies the upstream item universe. For Jira, it is derived from the normalized site URL because Jira issue keys are only unique inside one Jira organization. Two Jira Sources for the same site and different JQL views share an item file.

`source.hash` identifies the configured Source view and Action plan. Changing JQL or field mappings creates a new Source Snapshot identity. Changing Actions creates a different Action file without duplicating the Item Store.

Each `items-<source.slug>.snapshots` record commits one complete Source Snapshot.
The identity includes the configured Source ID, Source kind, and normalized Source
config. It excludes Actions. A failed or incomplete Run does not replace the
latest committed Snapshot. A committed empty Snapshot is valid.

Sources that share an Item Store keep separate Snapshot membership and their
source-specific normalized Item values.

Legacy `sources/<source-id>/items.jsonl` and `sources/<source-id>/actions.jsonl` files are not migrated automatically.

## Workspace lock

Normal `run` and non-dry `run --watch` acquire `run.lock` for the Workspace.

- `run --dry-run` skips the lock and does not write Store files.
- `run --watch --dry-run` skips the lock and does not write Store files.
- A non-dry watched Run holds the lock until it exits.
- Overlapping normal Runs for the same Workspace fail.

## `items-<source.slug>.jsonl`

Each line is one normalized item observation.

Each Item contains both identities:

- `id` — adapter-owned identity used for Store and Action matching.
- `reference_id` — provider-facing reference available to templates.

A new Run appends new observations. It does not rewrite older lines.

`list` reads the latest committed Source Snapshot for each configured Source.
It groups output in Workspace order and does not infer current membership from
legacy Item records. JSON output contains one object per Source and keeps
missing and ready-empty Snapshots distinct.

`show` keeps its output and item-reference rules. It now uses committed Source
Snapshot membership, so legacy Item observations without Snapshot boundaries are
not eligible for `show`.

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

## Action Plan Result

`list` and `dashboard` derive one result from the current rendered Action
identities for each Item:

- `success` — every current Action identity succeeded, or no Actions exist.
- `error` — an Action cannot render, or its latest attempt failed.
- `pending` — every other state, including cancelled attempts and changed
  rendered inputs.

A later successful attempt replaces an older failure for the same current
Action identity.

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

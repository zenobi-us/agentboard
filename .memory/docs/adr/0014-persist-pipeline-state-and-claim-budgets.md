# Persist pipeline state and claim budgets

## Status

Accepted

## Context

A Source Snapshot contains only Items that match the current Source query.
An Item can leave that query after an Action claims it. The Dashboard then loses
visibility of work that AgentBoard still owns.

The existing Source `limit` controls collection size. It does not control the
number of Items that a Run can claim.

## Decision

AgentBoard MUST keep external Item status separate from internal pipeline state.

AgentBoard MUST persist pipeline state for each configured Source, Item, and
Action plan. The identity MUST include the Workspace, Source, Item identity, and
Action plan hash.

The pipeline state vocabulary is:

- `claimed`
- `running`
- `succeeded`
- `failed`
- `cancelled`
- `stale`

The CLI runtime owns state transitions and persistence. Source and Action plugin
runtime contracts do not change.

A Workspace Source MAY define `pipeline.claim_limit`. This value limits new
`eligible` to `claimed` transitions in one Run. Source `limit` remains the
collection limit.

The Store MUST preserve the last Item snapshot for non-succeeded pipeline
executions. Dashboard views MUST merge these executions with the current Source
Snapshot.

A stale `claimed` or `running` execution MUST be visible after a process crash.
Recovery policy can mark it `stale` before a later retry.

## Consequences

Existing Source and Action plugins need no changes. The CLI Store, runtime, TUI,
and Workspace configuration contract change.

The same external Item can have separate pipeline executions in two Sources.
This supports implement and review pipelines that use the same GitHub issue.

A true time-based rate limiter is out of scope. Watch interval and
`pipeline.claim_limit` provide the first claim throttle.

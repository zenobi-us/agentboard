# Persist pipeline state and claim budgets

## Status

Accepted

## Context

A Source Snapshot contains only Items that match the current Source query.
An Item can leave that query after an Action claims it. The TUI then loses
visibility of work that AgentBoard still owns.

The existing Source `limit` controls collection size. It does not control the
number of Items that a Run can claim.

## Decision

AgentBoard MUST keep external Item status separate from internal pipeline state.

AgentBoard MUST persist pipeline state for each configured Source, Item, and
Action plan. The identity MUST include the Workspace, Source, Item identity, and
Action plan hash.

The pipeline state vocabulary is:

- `claimed`: the Item is reserved for this Run.
- `running`: Action launch was accepted, but the Action has no final result yet.
- `succeeded`: every Action in the plan completed successfully.
- `failed`: an Action completed with failure, or execution raised an error.
- `cancelled`: execution stopped by cancellation.
- `stale`: a `claimed` or `running` execution was not completed by the prior process.

`Action.execute()` MUST return a final `ActionResult` for synchronous work. An
asynchronous Action MUST return an explicit in-progress result. The result MAY
include a completion Promise when the Action can observe completion. The CLI
MUST persist final Action attempts only after completion. A launch accepted
without an observable completion remains `running` until recovery marks it
`stale` or a later execution records a final result.

The LLM Action observes completion for direct background launches through the
child process. Terminal launchers such as Herdr, Zellij, and tmux only report
that the terminal accepted the command. They remain `running` because the
terminal does not expose agent completion to the Action.

The CLI runtime owns state transitions and persistence. The optional in-progress
result extends the Action runtime contract without changing Source contracts.

A Workspace Source MAY define `pipeline.claim_limit`. This value limits new
Available Item to `claimed` transitions in one Action Run. Source `limit` remains
the collection limit. A Force Claim bypasses `pipeline.claim_limit` for one
Available Item.

An Item without a previous successful Action result remains eligible for a
later Action Run. This includes failed, cancelled, and stale executions.

The Store MUST preserve the last Item snapshot for every pipeline execution.
TUI views MUST merge these executions with the current Source Snapshot.
The TUI groups Items with no pipeline execution as `Available` and all
other pipeline Items as `Claimed`, under the next Action. The `claimed` state is
shown by the `Claimed` group, not by a child state glyph.

A stale `claimed` or `running` execution MUST be visible after a process crash.
Recovery policy marks it `stale` before a later retry. The TUI MUST label
`running` as completion pending and `stale` as disconnected.

## Consequences

Existing Source and Action plugins need no changes. The CLI Store, runtime, TUI,
and Workspace configuration contract change.

The same external Item can have separate pipeline executions in two Sources.
This supports implement and review pipelines that use the same GitHub issue.

A true time-based rate limiter is out of scope. Watch interval and
`pipeline.claim_limit` provide the first claim throttle.

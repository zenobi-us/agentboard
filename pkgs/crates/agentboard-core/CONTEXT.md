# AgentBoard Core Context

`agentboard-core` contains shared data structures used by the CLI, source crates, and action crates.

## Language

**Source**:
A configured provider of task-like items plus an optional query that selects which items match.
_Avoid_: Tracker, integration

**Item**:
A normalized local copy of one task-like record from a source.
_Avoid_: Ticket, issue, task when referring to the normalized AgentBoard record

**Action**:
A source-owned side effect that runs for a matching item when that item/action has no previous success record.
_Avoid_: Job, hook, plugin

**Action attempt**:
One recorded execution result for one item/action/rendered-input hash.
_Avoid_: Build result, task result

## Boundaries

- Core owns shared types and tiny cross-crate helpers only.
- Core must not depend on CLI orchestration, source crates, or action crates.
- Keep normalized `Item` small and store source-specific payloads in `raw`.
- Keep config enums boring and explicit; add variants only for implemented source kinds.

## ADRs

Read `.memory/docs/adr/pkgs/crates/agentboard-core/` before changing shared model names, config shape, or action identity fields.

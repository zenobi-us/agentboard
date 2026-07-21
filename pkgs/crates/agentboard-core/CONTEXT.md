# AgentBoard Core Context

`agentboard-core` contains shared data structures and internal registration contracts used by the CLI, source crates, and action crates.

## Language

**Source**:
A configured provider of task-like items plus an optional query that selects which items match.
_Avoid_: Tracker, integration

**Item**:
A normalized local copy of one task-like record from a source.
_Avoid_: Ticket, issue, task when referring to the normalized AgentBoard record

**Item identity**:
The adapter-owned, collision-resistant identity for an Item. It is exposed as `item.id`, is not configurable through field mapping, and is used by the Store and Action attempt identity.
_Avoid_: Display id, provider reference

**Item reference ID**:
The provider-native, human-facing identifier for an Item. It is exposed as `item.reference_id`, may be selected through `field_map.id`, and may require Source context to be globally unique, such as GitHub `10` or Jira `ABC-123`.
_Avoid_: Source id, Item identity

**Action**:
A source-owned side effect that runs for a matching item when that item/action has no previous success record.
_Avoid_: Job, hook, plugin

**Action attempt**:
One recorded execution result for one item/action/rendered-input hash.
_Avoid_: Build result, task result

## Boundaries

- Core owns shared types, Source and Action registration contracts, the internal Registry, and tiny cross-crate helpers.
- Core must not depend on CLI orchestration, source crates, or action crates.
- Registration is explicit and statically linked; Core does not define a runtime plugin ABI or community extension contract.
- Keep normalized `Item` small and store source-specific payloads in `raw`.
- Keep Workspace config envelopes boring and explicit while registered Source and Action crates own their typed configuration.

## ADRs

Read `.memory/docs/adr/pkgs/crates/agentboard-core/` before changing shared model names, config shape, or action identity fields.

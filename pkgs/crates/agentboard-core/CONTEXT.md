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

**Named Action**:
An Action with a Source-scoped identifier whose rendered inputs can be referenced by later Actions from the same Source.
_Avoid_: Step, named job

**Rendered Action**:
An Action whose inputs AgentBoard rendered for one Item. It is ready for Action runtime creation.
_Avoid_: Configured Action, Action attempt

**Prepared Action**:
The Workspace-scoped result of Action preparation for one configured Action. It owns shared resources and creates Action runtimes from rendered inputs.
_Avoid_: Action runtime, Action runtime factory

**Action Runtime**:
The Item-scoped executor that a Prepared Action creates for one Rendered Action.
_Avoid_: Prepared Action, Action attempt

**Rendered Action identity**:
The identity of one configured Action position after its inputs are rendered for an Item. It changes when those rendered inputs change.
_Avoid_: Action attempt, Action ID

**Action attempt**:
A recorded result for one Item and Rendered Action identity. It includes Action runtime creation and execution failures.
_Avoid_: Build result, task result

**Action attempt outcome**:
The result classification of an Action attempt: `success`, `failure`, or `cancelled`. A cancelled outcome records interrupted execution and does not satisfy the Action's previous-success rule.
_Avoid_: Success flag, exit status

## Boundaries

- Core owns shared types, Source and Action registration contracts, the internal Registry, and tiny cross-crate helpers.
- Core must not depend on CLI orchestration, source crates, or action crates.
- The retained Rust runtime uses explicit static registration. The Bun runtime resolves discovered Plugin Descriptors into Core configuration nodes.
- Keep normalized `Item` small and store source-specific payloads in `raw`.
- Keep Workspace config envelopes boring and explicit while registered Source and Action crates own their typed configuration.

## ADRs

Read `.memory/docs/adr/pkgs/crates/agentboard-core/` before changing shared model names, config shape, or action identity fields.

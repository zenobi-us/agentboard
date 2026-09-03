# XState as an application-state foundation for `apps/cli`

**Date:** 2026-08-22  
**Scope:** Evaluate XState v5 for AgentBoard CLI orchestration and operational state.  
**Decision status:** Research only. No dependency or code change is proposed by this document.

## Executive recommendation

AgentBoard SHOULD NOT replace the current `apps/cli/src/services/runtime.ts` orchestration with one large XState machine.

AgentBoard MAY use a small number of XState machines for explicit, long-lived workflows if the CLI adds more user-visible states or controls. The best first candidate is Watch Mode. Source collection and Action execution SHOULD remain ordinary TypeScript functions behind the existing Source and Action runtime interfaces.

The current runtime already has the required state boundaries:

```text
load Workspace -> lock -> collect Sources in parallel -> commit Snapshot -> execute Actions -> report
                                      |                         |
                                  cancellation              append-only Store
```

XState can model this flow, but it does not replace the Store, workspace lock, plugin interfaces, or append-only event records.

## Current CLI constraints

- `run` is the public pipeline command. Collection is an internal stage.
- `runWorkspace()` acquires one Workspace lock, then runs Sources with `Promise.allSettled()`.
- Each Source has a shared `AbortSignal` and can report `collecting`, `complete`, `failed`, or `cancelled` status.
- A successful Source collection appends a complete Source Snapshot. Failed or cancelled collection keeps the previous Snapshot authoritative.
- Action attempts are item-scoped and append-only. Cancelled Actions remain eligible for retry.
- Watch Mode repeats Runs while holding the Workspace lock and stops on cancellation.
- The Dashboard reads Store files. It does not execute a Run or share an in-memory runtime.
- `apps/cli` uses Bun and TypeScript. `apps/cli/package.json` does not currently depend on `xstate`.

Relevant decisions: [ADR 0012](../adr/0012-use-cooperative-cancellation-through-runtime-contexts.md), [CLI ADR 0001](../adr/apps/cli/0001-use-run-as-public-pipeline-command.md), [CLI ADR 0002](../adr/apps/cli/0002-store-items-and-actions-as-per-source-jsonl.md), and [CLI ADR 0003](../adr/apps/cli/0003-share-source-collection-status-with-tui.md).

## XState capabilities relevant to AgentBoard

### Actor model

A running state machine is an actor. Actors receive events, change state, invoke or spawn child actors, and emit snapshots. Invoked actors have a state-controlled lifecycle. Spawned actors have a transition-controlled lifecycle.

This maps well to a Run coordinator with one child actor per Source, and to a Watch coordinator that starts one Run per interval. Parallel states and dynamic spawning support the current parallel Source model without requiring one machine definition per configured Source.

Sources: [XState actors](https://stately.ai/docs/actors), [Spawn](https://stately.ai/docs/spawn), [Parallel states](https://stately.ai/docs/parallel-states)

### Async work and cancellation

`fromPromise()` creates promise actors for asynchronous work. Promise actors receive an `AbortSignal`; stopping the actor aborts the signal. Invoked actors stop when their parent state exits. If an invoked Promise settles after its state exits, XState discards the result.

This is useful for explicit lifecycle ownership. It does not remove the need to pass the signal into Source and Action implementations. AgentBoard's existing `AbortSignal` contract is compatible with this model.

Sources: [Promise actors](https://stately.ai/docs/promise-actors), [Invoke](https://stately.ai/docs/invoke)

### Deterministic transitions and polling

XState transitions are deterministic for a given state and event. Guards select valid transitions. This can make cancellation, retry, failure, and watch-control rules easier to inspect than nested conditionals.

Delayed transitions can model the Watch Mode interval. Their timers are cancelled when the state exits. Callback actors can model terminal input or other event sources if Dashboard controls become more complex.

Sources: [Transitions](https://stately.ai/docs/transitions), [Delayed transitions](https://stately.ai/docs/delayed-transitions), [Callback actors](https://stately.ai/docs/callback-actors)

### Persistence and recovery

Actors expose `getPersistedSnapshot()` and can restore from a persisted snapshot. XState restores nested invoked and spawned actors recursively. Persisted state is JSON-serializable and is not the same as the last emitted snapshot. Invocations restart during restoration. Already-executed actions do not run again. XState documents event sourcing as a better fit when action replay matters.

This is not a direct replacement for AgentBoard's Store. The Store records source observations, snapshot boundaries, and action attempts. XState persistence would describe control state, not authoritative item membership or action history. Persisting an in-flight Run would also need careful rules because Source Snapshot commit and Action side effects are external operations.

Source: [Persistence](https://stately.ai/docs/persistence)

### Testing

XState tests create actors, send events, and assert snapshots. XState also includes model-based testing utilities under `xstate/graph`; the old standalone `@xstate/test` package is deprecated.

This could improve tests for Watch Mode and cancellation paths. It would add less value to pure Source and Action functions, which are already tested through direct calls and integration tests.

Source: [Testing](https://stately.ai/docs/testing)

### Runtime and TypeScript fit

The core `xstate` package has zero dependencies and is documented as running anywhere JavaScript runs. XState v5 requires TypeScript 5.0 or newer. The repository currently uses TypeScript 6.0.3 in `apps/cli`, so the stated TypeScript requirement is satisfied.

The npm package page reports `xstate` version `5.32.5` at research time. Bun compatibility is therefore expected from the package's JavaScript runtime model, but this repository SHOULD verify it with a small Bun build and test before adoption.

Sources: [Installation](https://stately.ai/docs/installation), [TypeScript](https://stately.ai/docs/typescript), [npm package metadata](https://www.npmjs.com/package/xstate)

## Fit by responsibility

| Responsibility | Fit | Recommendation |
|---|---:|---|
| Watch Mode lifecycle | High | Good first adoption target. Model `idle`, `running`, `waiting`, `cancelled`, and `failed` explicitly. |
| One Run coordinator | Medium | Possible, but current function is small enough. Add only if more control events or visible progress states appear. |
| Per-Source collection | Medium | Promise actors fit, but Source plugins already own async behavior and cancellation. Avoid wrapping every call without a concrete benefit. |
| Per-Item Action execution | Low to medium | Many short-lived actors could add ceremony. Keep item execution in the existing loop unless concurrency, retry, or pause controls become complex. |
| Dashboard refresh and input | Medium | A local actor could model refresh and quit events, but the current timer and raw input loop are simple. No immediate need. |
| Store authority and recovery | Low | XState persistence cannot replace append-only JSONL, snapshot boundaries, or action-attempt semantics. |
| Plugin registration and configuration | Low | Existing static descriptors and validated configuration are a separate concern. |

## Main risks

1. **Two state systems.** XState snapshots and Store status files could diverge. The Store MUST remain authoritative for persisted Source and Action state.
2. **Side-effect semantics.** XState actions are transition effects, not transactions. A machine cannot roll back an already-written Snapshot or an external Action.
3. **Over-modeling.** A machine for every helper would increase code and obscure the existing Source and Action boundaries.
4. **Persistence mismatch.** Restoring a control snapshot can restart invocations, while AgentBoard must avoid duplicate external side effects and preserve retry rules.
5. **Concurrent failure policy.** `Promise.allSettled()` gives item/source-scoped failure behavior today. A parent machine would need explicit aggregation rules to preserve that policy.
6. **Operational dependency.** XState is mature, but it is still a new runtime dependency for the CLI. Bun build, test, and generated CLI output MUST be checked before use.

## Smallest useful adoption plan

1. Add `xstate` only in a branch or spike. Do not change Source or Action interfaces.
2. Model Watch Mode as one machine with these states: `idle`, `running`, `waiting`, `cancelled`, and `failed`.
3. Invoke the existing `runWorkspaceUnlocked` or an equivalent narrow coordinator through `fromPromise()`. Pass the existing `AbortSignal` into plugin runtimes.
4. Keep workspace lock ownership outside the machine, or make lock ownership one clearly bounded invocation. Do not persist lock state.
5. Emit Store status through the existing Store functions. Do not make XState snapshots the Dashboard data source.
6. Test cancellation during Source collection, cancellation during the watch interval, one failed Source among successful Sources, and Action retry after a cancelled attempt.
7. Compare the machine version against the current tests. Keep XState only if it reduces branching or adds a required control feature.

## Conclusion

XState is a technically compatible tool for explicit workflow control in `apps/cli`. It is not a better general-purpose Store or a replacement for the current runtime interfaces. The practical choice is selective adoption: use XState for a root workspace-run coordinator when explicit Source, Action, retry, cancellation, or polling states provide a clear benefit. Watch Mode is the smallest first slice. Keep the existing ordinary TypeScript functions and JSONL Store as the implementation boundary for side effects and durable records until a Bun spike proves the machine reduces complexity.

# TUI XState machine review

## Scope

This review traces `apps/cli/src/tui/` and proposes a hierarchy in which machines own TUI behavior. It does not change source code.

## Current flow

`apps/cli/src/cli/tui.ts` calls `startTui()`. `main.tsx` owns OpenTUI renderer creation and destruction. `App` owns React provider composition. `AppScreen` owns route rendering and runtime effects.

```text
startTui
  -> App
    -> AppMachineProvider
      -> tuiMachine
        -> loadWorkspace actor
        -> active state
          -> itemRun actor
    -> AppScreen
      -> React effects run workspace and item work
      -> route selects a view
        -> view creates a local machine actor
```

`AppScreen` owns workspace run execution in React effects. It creates plugin runtimes, creates an `AbortController`, calls `runWorkspace` or `watchWorkspace`, stores items and results in React state, and sends completion commands. See `apps/cli/src/tui/App.tsx:36-111`. Because the effect depends on the full `runRequest`, changing `stopping` can rerun the effect instead of stopping one owned invocation.

`AppScreen` also runs item work in a second React effect. The app machine already invokes `itemRunMachine`, so one item run can start twice. See `apps/cli/src/tui/App.tsx:113-145` and `apps/cli/src/tui/services/app/machine.ts:102-107,221-245`.

The app machine owns workspace loading, route data, run request flags, and item-run result storage. It does not own workspace execution. See `apps/cli/src/tui/services/app/machine.ts:26-53,79-107,108-260`.

`workspaceMachine` owns source selection and route commands, but it contains placeholder item data and receives only source IDs. See `apps/cli/src/tui/services/workspace/workspace.machine.ts:4-10,26-31,51-65`.

`WorkspaceView` creates that machine and sends mouse clicks directly to the app actor. The machine does not own the click path. See `apps/cli/src/tui/components/workspace/workspace-view.tsx:20-24,35-50`.

`sourceMachine` owns item index movement and keyboard navigation. `SourceView` supplies items from props, but the actor input is not a machine event or a parent-owned child actor. The view sends breadcrumb navigation directly to the app actor. See `apps/cli/src/tui/services/source/source.machine.ts:4-16,41-69` and `apps/cli/src/tui/components/workspace/source-view.tsx:22-49`.

`itemViewMachine` owns only Escape and item-run command translation. `ItemView` and `ActionItemView` create it locally. Their mouse handlers send route events directly to the app actor. See `apps/cli/src/tui/services/item/item.view.machine.ts:7-47` and `apps/cli/src/tui/components/workspace/item-view.tsx:17-25,76-88`.

`itemRunMachine` is the only execution machine. It invokes `runItem` and reports to its parent. The parent already invokes it, but `AppScreen` duplicates the same work outside XState. See `apps/cli/src/tui/services/item/item.view.machine.ts:49-103`. The component sends `COMMAND` values such as `item.run-complete`, while the app machine handles `ITEM_RUN_COMPLETE` events. Those completion commands do not match. Also, workspace loading passes `false` to `loadWorkspace`, so the loaded Workspace does not contain Action runtimes for the item-run actor. See `apps/cli/src/tui/services/app/machine.ts:62-65`.

`modalMachine` is created inside `SettingsModal`. The component sends `MODAL_CLOSED` during render when the child reaches `closed`. `settings.save` has no transition and settings has no state owner. See `apps/cli/src/tui/components/app/settings-modal.tsx:7-18` and `apps/cli/src/tui/services/app/modal.machine.ts:3-22`.

`ItemsView` has no machine. It flattens source items and renders them. See `apps/cli/src/tui/components/workspace/items-view.tsx:5-29`.

The keymap provider is a React registry. It selects the last registered scope and sends a generic `COMMAND` event to that actor. This is useful infrastructure, but scope lifetime and event routing remain component-owned. See `apps/cli/src/tui/services/keymaps.tsx:18-83`. The registry gives priority to the most recently registered scope, so React mount order defines input routing.

The TUI has no focused machine tests. Existing tests cover CLI and runtime services, not the TUI state graph. The TUI README is a light design note, not an architecture contract. See `apps/cli/src/tui/README.md:1-28`.

## Main mess

- React owns run lifecycles, cancellation, errors, and result storage.
- The stop transition changes `runRequest`, which can retrigger the React run effect instead of stopping an invoked operation.
- Item-run completion events use two incompatible event shapes.
- The item-run actor receives a Workspace loaded without Action runtimes.
- XState owns the item run while React starts the same item run again.
- Components create child actors instead of the app machine invoking them.
- Components send route events directly to the root actor.
- `workspaceMachine` has placeholder item data.
- `sourceMachine` and other view machines receive data through one-time actor input.
- Derived action summaries and output status live in views.
- `SettingsModal` reports child completion as a render side effect.
- `ItemsView` has no state owner.
- `AppRoute` is context data instead of a hierarchical state value.

## Proposed hierarchy

```text
tui
├── booting
│   └── invoke loadWorkspace
├── failed
├── ready                       parallel
│   ├── navigation              compound
│   │   ├── workspace
│   │   ├── items
│   │   ├── source
│   │   ├── item
│   │   └── actionItem
│   ├── execution               compound
│   │   ├── idle
│   │   ├── listing / running / watching
│   │   ├── stopping
│   │   └── itemRunning
│   └── overlay                 compound
│       ├── none
│       └── settings
└── exiting
```

The root machine owns the loaded Workspace, Source Snapshots, Action results, errors, selected IDs, and run status. It invokes all runtime actors. It assigns runtime results from actor completion events.

The navigation region owns route transitions. Route data becomes state-node context only where needed, such as `source.sourceId` or `item.itemId`. Child view machines can remain internal invoked actors, but they MUST communicate with the parent through typed events. They MUST NOT receive `AnyActorRef` or call `appActor.send`.

The workspace child owns source selection and opens a Source by sending `OPEN_SOURCE`. It receives the real source list and item index data from the parent. The source child owns item selection and sends `OPEN_ITEM` or `BACK`. The item child owns `RUN_ITEM`, `BACK`, and action selection. The action-item child owns `BACK` only.

The execution region invokes one workspace run actor for `list`, `run`, or `watch`. The actor creates plugin runtimes, receives the invocation `AbortSignal`, reports source results, and stops on state exit. The root machine removes the `AppScreen` workspace effect. The execution Workspace adapter MUST create both Source and Action runtimes with the same signal.

The execution region invokes one item-run actor. The root machine sends it the selected item. The root machine removes the `AppScreen` item effect. This removes the current duplicate item run.

The overlay region invokes `modalMachine` from the `settings` state. The modal sends `CLOSE` or `SAVE` to its parent. The component only renders the child snapshot and sends input events. It does not send a parent event during render.

The components become projections:

- Select a snapshot from the relevant actor.
- Render data and derived machine selectors.
- Send semantic events on keyboard or mouse input.
- Do not call runtime functions.
- Do not create `AbortController` instances.
- Do not store domain data in React state.
- Do not send route events to a root actor directly.

## Migration order

1. Move workspace run execution from `AppScreen` into an invoked `workspaceRunMachine`.
2. Make operation state own cancellation so `STOP_REQUESTED` exits the invocation instead of changing an effect dependency.
3. Remove the duplicate item effect and make `itemRunMachine` the only item-run actor.
3. Change root routing from `AppRoute` assignments to a compound `navigation` state.
4. Invoke workspace, source, item, and action-item machines from the navigation region.
5. Replace `appActor` references with parent events.
6. Move source items and run results into root machine context.
7. Move action-result derivation into machine selectors or pure machine-owned output data.
8. Move settings modal ownership into the overlay region.
9. Convert `ItemsView` into a pure projection of the `items` navigation state.
10. Add machine tests for routing, cancellation, run exclusivity, and item-run completion.

## Focused recommendation: make `AppScreen` a projection

`AppScreen` should not coordinate TUI behavior. It should select the root snapshot and choose a renderer. The root machine should expose all state needed by those renderers.

Move these values out of React state:

- `sourceItems` -> root `data` context.
- `sourceRuns` -> root `data` context.
- `runError` -> root `error` context.
- `runRequest` -> operation state nodes, not a `{ mode, id, stopping }` record.
- `itemRunRequest` -> item-run state context.
- `route` -> navigation state nodes.

Move these effects into invoked actors:

- Workspace runtime construction and `runWorkspace` / `watchWorkspace` -> `workspaceOperationMachine`.
- Item runtime construction and `runItem` -> `itemRunMachine`.
- Result merging -> root machine actions on typed completion events.
- Abort handling -> invocation lifetime. Exiting `running`, `watching`, or `itemRunning` cancels the actor.

Then `AppScreen` only performs this work:

```tsx
const snapshot = AppMachineContext.useSelector((state) => state)
return <RouteView snapshot={snapshot} />
```

A practical root shape is:

```text
tui
├── booting                 invoke loadWorkspace
├── ready                   parallel
│   ├── navigation          compound route states
│   ├── workspaceOperation  idle | listing | running | watching | stopping
│   ├── itemOperation       idle | running
│   └── overlay             none | settings
├── failed
└── exiting
```

The operation machine can remain an ordinary XState actor. It does not need a separate React hook or a component effect. It receives the loaded Workspace as input, creates the execution Workspace with one `AbortSignal`, and emits `SOURCE_RESULT`, `WORKSPACE_COMPLETE`, `WORKSPACE_FAILED`, or `WORKSPACE_STOPPED`.

The root machine owns the event protocol:

```text
app.run       -> workspaceOperation.running
app.stop      -> workspaceOperation.stopping
run result    -> data.sourceItems and data.sourceRuns
item.run      -> itemOperation.running
item result   -> data.sourceRuns
route intent  -> navigation state transition
```

This removes the most important leak: `AppScreen` no longer decides when work starts, how it stops, how results merge, or which error event to send.

## Required invariants

- A workspace run MUST have one owner and one cancellation signal.
- An item run MUST have one owner and one active invocation.
- A view MUST NOT call a runtime function.
- A view MUST NOT send a root route event directly.
- Input priority MUST NOT depend on accidental React mount order.
- A child machine MUST communicate with its parent through typed events.
- A run result MUST enter machine context before a view can render it.
- Closing a modal MUST occur through a machine transition, not during render.

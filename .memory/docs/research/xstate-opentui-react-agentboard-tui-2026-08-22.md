# XState v5 with OpenTUI React for an AgentBoard TUI subcommand

**Date:** 2026-08-22  
**Scope:** Evaluate XState v5 with `@opentui/react` for a new AgentBoard TUI subcommand.  
**Decision status:** Research only. No dependency or source change is proposed.

## Executive summary

XState v5 and OpenTUI React have compatible roles. XState can own application state and workflow events. OpenTUI can own terminal setup, input, layout, rendering, and terminal cleanup. React connects the two through hooks.

The official sources do not state that XState v5 and `@opentui/react` have a tested integration. Treat the combination as technically plausible, not proven. Run a small Bun spike before adoption.

The smallest useful design is one TUI actor for screen state and commands. Keep AgentBoard collection, Store writes, workspace locks, and action execution in existing service functions. Send service results into the actor as typed events. Do not make terminal renderables or durable Store records part of XState context.

## Key findings

### 1. Integration pattern

`@opentui/react` provides `createRoot(renderer)` and renders a React tree into an existing OpenTUI `CliRenderer`. The renderer owns the terminal session and input boundary. XState provides actors and React hooks.

Use `useActorRef(machine)` when the component needs a stable actor reference. Use `useSelector(actorRef, selector)` for small state slices. Use `useActor(machine)` only when the component needs the full snapshot and event sender. XState documents that `useActorRef` avoids rerenders from every actor update, while `useSelector` rerenders when the selected value changes.

OpenTUI uses React JSX intrinsic elements that map to OpenTUI renderables. The TUI component can therefore read selected XState state and render it like other React state.

**Sources:**

- [XState React bindings](https://stately.ai/docs/xstate-react)
- [OpenTUI React bindings](https://opentui.com/docs/bindings/react/)
- [OpenTUI renderer](https://opentui.com/docs/core-concepts/renderer/)

### 2. Lifecycle and render loop

The application must create one `CliRenderer`, render one React root, and keep the process alive until the user exits or the application fails. OpenTUI owns frame scheduling and terminal input. The application must call `renderer.destroy()` during normal shutdown and failure cleanup.

`root.unmount()` removes the React tree and runs React effect cleanup. It does not destroy the renderer. `renderer.destroy()` releases terminal modes, listeners, timers, renderables, and native resources. A `try`/`finally` boundary is required around the application lifetime.

XState actor lifetime must match the TUI root lifetime. Start the actor through `useActorRef` or `useActor`. Stop or dispose any actor that the application creates outside React before renderer destruction. Do not let a background actor continue after the terminal closes.

**Source:** [OpenTUI lifecycle and cleanup](https://opentui.com/docs/core-concepts/lifecycle/)

### 3. Input handling

OpenTUI provides two input paths:

- Use `useKeyboard` or component key handlers for UI-local commands.
- Use `renderer.keyInput` listeners for a small global command set, such as quit or reload.

The keyboard hook registers after mount and removes its listener during React effect cleanup. Direct renderer listeners run before the focused renderable and need explicit removal. Global handlers can call `stopPropagation()` or `preventDefault()` when they own the key.

A TUI component must translate terminal input into XState events. The machine must decide whether the command is valid in the current state. Do not put workflow policy inside key handlers.

OpenTUI has a separate Ctrl+C path. Its default signal and key handling can destroy the renderer. If AgentBoard needs cooperative cancellation, disable the default Ctrl+C exit path and route Ctrl+C to an XState `CANCEL` event. The application must then destroy the renderer after the actor reaches its shutdown state.

**Sources:**

- [OpenTUI keyboard input](https://opentui.com/docs/core-concepts/keyboard/)
- [OpenTUI lifecycle and cleanup](https://opentui.com/docs/core-concepts/lifecycle/)
- [OpenTUI React bindings](https://opentui.com/docs/bindings/react/)

### 4. Cancellation and async work

XState `fromPromise()` creates promise actors. The actor receives an `AbortSignal`. Stopping the actor aborts the signal and discards the resolved or rejected result. This matches AgentBoard's existing cooperative cancellation contract.

Use one invoked promise actor for one bounded operation, such as loading the initial dashboard data or running one workspace operation. Pass the signal into existing Source and Action runtime functions. Do not wrap every small helper in an actor.

For terminal events, XState callback actors can receive events and send events to the parent. OpenTUI can remain the input owner while the callback actor is useful for a long-lived external event source. A callback actor cannot spawn actors and does not produce output, so it is not a general replacement for the TUI component tree.

The machine must define the result of cancellation. AgentBoard must preserve current rules for source-scoped failures, authoritative snapshots, append-only action attempts, and action retry. XState cancellation stops control flow. It does not undo Store writes or external actions.

**Sources:**

- [XState promise actors](https://stately.ai/docs/promise-actors)
- [XState callback actors](https://stately.ai/docs/callback-actors)
- [XState persistence](https://stately.ai/docs/persistence)
- [AgentBoard cancellation ADR](../adr/0012-use-cooperative-cancellation-through-runtime-contexts.md)

### 5. Testing

OpenTUI provides an in-memory test renderer. `createTestRenderer()` supplies a renderer, mock keyboard input, frame helpers, and captured character output. Tests must destroy the renderer in teardown. `@opentui/react/test-utils` provides `testRender(node, options)` for React-aware rendering.

XState tests can create actors, send events, and assert snapshots. The `xstate/graph` entry point provides path generation and model-based testing. The deprecated standalone `@xstate/test` package must not be added.

Use two test layers:

1. Test the XState machine with Bun's test runner. Cover command guards, cancellation, failure, retry, and shutdown transitions.
2. Test the OpenTUI screen with the OpenTUI test renderer. Send keys through `mockInput`, render a frame, and assert visible output.

Add a small integration test only after the first two layers pass. It must mount the real TUI root, inject a fake service actor or service function, press a key, and assert the resulting frame.

**Sources:**

- [OpenTUI testing](https://opentui.com/docs/core-concepts/testing/)
- [OpenTUI React bindings](https://opentui.com/docs/bindings/react/)
- [XState testing](https://stately.ai/docs/testing)
- [XState graph and paths](https://stately.ai/docs/graph)
- [Bun test runner](https://bun.sh/docs/test)

### 6. Packaging and runtime

OpenTUI React requires React `>=19.2.0`. OpenTUI's runtime support page lists Bun `1.3.0` or later for Core. OpenTUI Core loads a matching optional native package for the platform. The package publishes platform-specific Core artifacts for Linux, macOS, and Windows.

This creates a larger packaging surface than a normal React component. A released AgentBoard binary or package must include the correct optional native artifact and test the target platform. The TUI must not load `@opentui/core` during commands that do not need the TUI.

OpenTUI documents Bun support and Bun-specific runtime modules. Bun can bundle a Bun-targeted entry point and run TypeScript directly. Bun's bundler does not remove the need to test native package resolution in the intended distribution form.

XState's React integration is a separate package from the core `xstate` package. The TUI dependency set is therefore at least `xstate`, `@xstate/react`, `@opentui/core`, `@opentui/react`, and React. Use the repository's existing dependency policy and verify exact versions in the spike.

**Sources:**

- [OpenTUI runtime and platform support](https://opentui.com/docs/getting-started/runtime-support/)
- [OpenTUI React bindings](https://opentui.com/docs/bindings/react/)
- [Bun bundler](https://bun.sh/docs/bundler)
- [Bun JSX](https://bun.sh/docs/runtime/jsx)
- [XState React bindings](https://stately.ai/docs/xstate-react)

## Recommended architecture

```text
terminal input -> OpenTUI key handler -> XState event
                                      |
                              TUI actor state
                                      |
React selectors <- OpenTUI React tree <- service result events
                                      |
                    existing runtime, Store, and action boundaries
```

Use these boundaries:

- **OpenTUI:** terminal setup, keyboard input, focus, layout, frames, and cleanup.
- **React:** component composition and selected state rendering.
- **XState:** screen states, command transitions, cancellation, loading, and error presentation.
- **Existing AgentBoard runtime:** source collection, workspace locking, Store writes, action execution, and durable records.

The first machine can use states such as `loading`, `ready`, `running`, `cancelling`, `error`, and `exiting`. Keep service data in existing domain types. Store only the minimum view state and operation identifiers in machine context.

## Decision criteria

Adopt the combination only if the Bun spike proves all of these conditions:

1. The TUI starts and exits without leaving raw mode or the alternate screen active.
2. Ctrl+C cancels the active operation instead of killing the process before cleanup.
3. `fromPromise()` passes cancellation to the existing runtime and does not report stale results after cancellation.
4. `@opentui/react/test-utils` can render the root and assert keyboard-driven state changes.
5. Bun resolves OpenTUI's native package in the supported development and distribution forms.
6. The machine reduces branching or adds a required control feature. A machine that only replaces local React state is not enough.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| No official source confirms this exact XState/OpenTUI combination. | Run a focused integration spike before changing the CLI. Keep the first machine small. |
| Two state systems can diverge. | Keep the Store authoritative for durable source and action state. Treat XState as control and view state. |
| Renderer cleanup can race with async services. | Route shutdown through one actor event. Stop actors, await bounded service cleanup, then call `renderer.destroy()` in `finally`. |
| Ctrl+C can bypass application cancellation. | Configure OpenTUI signal and Ctrl+C behavior explicitly. Test both key and signal paths. |
| Native optional packages can fail in distribution. | Build and run the TUI on every supported target. Do not import TUI modules from non-TUI commands. |
| Over-modeling creates more code. | Start with one screen actor. Keep Source and Action functions ordinary. |
| Persisted XState state can restart invocations or skip completed actions. | Do not persist in-flight machine state until duplicate side effects have a documented recovery rule. |

## Recommendation

Use XState v5 with OpenTUI React only for explicit TUI control state. Start with one machine and one screen. Keep AgentBoard's runtime and Store as the side-effect and durability boundary.

Do not add XState to the non-TUI CLI path. Do not model each Source or Action as a separate actor unless the TUI needs pause, retry, progress, or per-item control.

Build the smallest Bun spike before adoption. If the spike passes the decision criteria and the machine removes real branching, keep the design. Otherwise use React state and existing AgentBoard functions.

## Revisit conditions

Revisit this decision when AgentBoard needs any of these features:

- Pause, resume, retry, or cancel controls for a running operation.
- Multiple TUI screens with explicit navigation states.
- Watch mode or long-lived refresh control inside the TUI.
- A need for model-based coverage of many command paths.
- A supported binary distribution that exposes native package loading problems.

The existing XState research also recommends selective adoption, with Watch Mode as the smallest first slice. See [XState as an application-state foundation](./xstate-cli-state-foundation-2026-08-22.md).

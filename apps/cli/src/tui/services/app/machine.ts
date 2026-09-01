import { assign, fromPromise, sendTo, setup } from "xstate"
import type { Item } from "@clankpipe/core/config"
import type { LoadedWorkspace } from "../../../services/config/workspace.ts"
import { runWorkspace, watchWorkspace, type SourceRunResult, type WorkspaceRunResult } from "../../../services/runtime.ts"
import { loadWorkspace } from "../../../services/workspace.ts"
import { itemRunMachine } from "../item/item.view.machine.ts"

/** Identify the view that the navigation region must render. */
export type AppRoute =
  | { name: "workspace" }
  | { name: "items" }
  | { name: "source"; sourceId: string }
  | { name: "item"; sourceId: string; itemId: string }
  | { name: "action-item"; sourceId: string; itemId: string; actionIndex: number }

/** Describe the operation shown by global controls. */
type RunRequest = {
  mode: "idle" | "run" | "watch" | "list"
  id: number
  stopping: boolean
}

/** Identify the item sent to the item operation actor. */
type ItemRunRequest = {
  sourceId: string
  itemId: string
}

/** Store all data that views need from the root actor. */
type MachineContext = {
  message: string
  route: AppRoute
  workspacePath: string
  version: string
  runRequest: RunRequest
  itemRunRequest?: ItemRunRequest
  workspaceConfig?: unknown
  executableWorkspace?: LoadedWorkspace
  error?: string
  itemRunError?: string
  runError?: string
  sourceRuns: Record<string, SourceRunResult>
  sourceItems: Record<string, readonly Item[]>
}

/** Define every event accepted by the root TUI machine. */
type MachineEvents =
  | { type: "COMMAND"; code: string }
  | { type: "ROUTE_WORKSPACE" }
  | { type: "ROUTE_ITEMS" }
  | { type: "ROUTE_SOURCE"; sourceId: string }
  | { type: "ROUTE_ITEM"; sourceId: string; itemId: string }
  | { type: "ROUTE_ACTION_ITEM"; sourceId: string; itemId: string; actionIndex: number }
  | { type: "REFRESH" }
  | { type: "QUIT" }
  | { type: "MODAL_CLOSED" }
  | { type: "RUN_ITEM"; sourceId: string; itemId: string; item: Item }
  | { type: "ITEM_RUN_COMPLETE"; result: SourceRunResult }
  | { type: "ITEM_RUN_FAILED"; error: string }
  | { type: "SOURCE_RUNS_UPDATED"; runs: Record<string, SourceRunResult> }
  | { type: "WORKSPACE_COMPLETE"; result: WorkspaceRunResult }
  | { type: "WORKSPACE_FAILED"; error: string }

/** Create the root actor that owns TUI lifecycle, data, navigation, and operations. */
export const tuiMachine = setup({
  types: {
    input: {} as { workspacePath: string; version: string },
    context: {} as MachineContext,
    events: {} as MachineEvents,
  },
  actors: {
    /** Load the Workspace without starting a run. */
    loadWorkspace: fromPromise(({ input, signal }: { input: { workspacePath: string }; signal: AbortSignal }) =>
      loadWorkspace(input.workspacePath, undefined, signal, true),
    ),
    /** Run one item operation and report its result to the parent actor. */
    itemRun: itemRunMachine,
    /** Run one Workspace operation owned by the active operation state. */
    workspaceRun: fromPromise(async ({ input, signal }: {
      input: { workspace: LoadedWorkspace; mode: "list" | "run" | "watch" }
      signal: AbortSignal
    }): Promise<WorkspaceRunResult> => {
      /** Attach the invocation signal to every runtime input. */
      const workspace = {
        ...input.workspace,
        cancellation: signal,
        sources: input.workspace.sources.map((source) => ({ ...source, cancellation: signal })),
      }
      /** Execute the requested operation through the existing runtime service. */
      return input.mode === "watch"
        ? watchWorkspace(workspace)
        : runWorkspace(workspace, { dryRun: input.mode === "list" })
    }),
  },
}).createMachine({
  id: "tui",
  initial: "initialising",
  /** Initialize root context before the first actor starts. */
  context: ({ input }) => ({
    message: "Ready",
    route: { name: "workspace" },
    workspacePath: input.workspacePath,
    version: input.version,
    runRequest: { mode: "idle", id: 0, stopping: false },
    sourceRuns: {},
    sourceItems: {},
  }),
  states: {
    /** Load the Workspace before rendering the active TUI. */
    initialising: {
      entry: assign({ message: "Initializing...", error: undefined }),
      invoke: {
        src: "loadWorkspace",
        input: ({ context }) => ({ workspacePath: context.workspacePath }),
        onDone: {
          target: "active.listing",
          actions: assign({
            workspaceConfig: ({ event }) => event.output.config,
            executableWorkspace: ({ event }) => event.output,
            message: "Listing...",
            runRequest: { mode: "list", id: 1, stopping: false },
          }),
        },
        onError: {
          target: "error",
          actions: assign({
            error: ({ event }) => String(event.error),
            message: "Workspace load failed",
          }),
        },
      },
    },
    /** Hold navigation, operation, and overlay states after loading. */
    active: {
      /** Start with the default list operation. */
      initial: "listing",
      states: {
        /** Hold the TUI after an operation completes. */
        idle: {},
        /** Collect the current Source items without running Actions. */
        listing: {
          invoke: {
            src: "workspaceRun",
            input: ({ context }) => ({ workspace: context.executableWorkspace!, mode: "list" as const }),
            onDone: { target: "../idle", actions: assign(({ event }) => workspaceResultContext(event.output)) },
            onError: { target: "../idle", actions: assign(({ event }) => workspaceErrorContext(event.error)) },
          },
        },
        /** Run the full Workspace pipeline once. */
        running: {
          invoke: {
            src: "workspaceRun",
            input: ({ context }) => ({ workspace: context.executableWorkspace!, mode: "run" as const }),
            onDone: { target: "../idle", actions: assign(({ event }) => workspaceResultContext(event.output)) },
            onError: { target: "../idle", actions: assign(({ event }) => workspaceErrorContext(event.error)) },
          },
        },
        /** Watch the Workspace until the actor is stopped. */
        watching: {
          invoke: {
            src: "workspaceRun",
            input: ({ context }) => ({ workspace: context.executableWorkspace!, mode: "watch" as const }),
            onDone: { target: "../idle", actions: assign(({ event }) => workspaceResultContext(event.output)) },
            onError: { target: "../idle", actions: assign(({ event }) => workspaceErrorContext(event.error)) },
          },
        },
      },
      on: {
        /** Route global commands to the active child state. */
        COMMAND: [
          { guard: ({ event }) => event.code === "app.quit", target: "exiting" },
          { guard: ({ event, context }) => event.code === "app.run" && context.runRequest.mode === "idle", target: ".running", actions: assign(({ context }) => ({ message: "Running...", runRequest: { mode: "run" as const, id: context.runRequest.id + 1, stopping: false } })) },
          { guard: ({ event, context }) => event.code === "app.watch" && context.runRequest.mode === "idle", target: ".watching", actions: assign(({ context }) => ({ message: "Watching...", runRequest: { mode: "watch" as const, id: context.runRequest.id + 1, stopping: false } })) },
          { guard: ({ event, context }) => event.code === "app.list" && context.runRequest.mode === "idle", target: ".listing", actions: assign(({ context }) => ({ message: "Listing...", runRequest: { mode: "list" as const, id: context.runRequest.id + 1, stopping: false } })) },
          { guard: ({ event }) => event.code === "app.stop", target: ".idle", actions: assign({ message: "Stopping...", runRequest: { mode: "idle" as const, id: 0, stopping: false } }) },
          { guard: ({ event }) => event.code === "app.open-settings", target: "settings" },
        ],
        /** Update the status message without starting domain work. */
        REFRESH: { actions: assign({ message: "Refreshed" }) },
        /** Navigate to the Workspace view. */
        ROUTE_WORKSPACE: { actions: assign({ route: { name: "workspace" } }) },
        /** Navigate to the all-items view. */
        ROUTE_ITEMS: { actions: assign({ route: { name: "items" } }) },
        /** Navigate to one Source view. */
        ROUTE_SOURCE: { actions: assign(({ event }) => ({ route: { name: "source", sourceId: event.sourceId } })) },
        /** Navigate to one Item view. */
        ROUTE_ITEM: { actions: assign(({ event }) => ({ route: { name: "item", sourceId: event.sourceId, itemId: event.itemId } })) },
        /** Navigate to one Action result view. */
        ROUTE_ACTION_ITEM: { actions: assign(({ event }) => ({ route: { name: "action-item", sourceId: event.sourceId, itemId: event.itemId, actionIndex: event.actionIndex } })) },
        /** Forward an Item run request to the item actor. */
        RUN_ITEM: {
          guard: ({ context }) => context.itemRunRequest === undefined,
          actions: [
            sendTo("itemRunner", ({ event }) => ({ type: "RUN_ITEM", sourceId: event.sourceId, item: event.item })),
            assign(({ event }) => ({ itemRunError: undefined, itemRunRequest: { sourceId: event.sourceId, itemId: event.itemId }, message: "Running item..." })),
          ],
        },
        /** Store one completed Item run. */
        ITEM_RUN_COMPLETE: {
          actions: assign(({ context, event }) => ({ itemRunRequest: undefined, itemRunError: undefined, sourceRuns: { ...context.sourceRuns, [event.result.id]: event.result }, message: "Ready" })),
        },
        /** Store one failed Item run. */
        ITEM_RUN_FAILED: {
          actions: assign(({ event }) => ({ itemRunRequest: undefined, itemRunError: event.error, message: "Item run failed" })),
        },
        /** Merge externally supplied Source results. */
        SOURCE_RUNS_UPDATED: {
          actions: assign(({ context, event }) => ({ sourceRuns: { ...context.sourceRuns, ...event.runs } })),
        },
        /** Leave the active state and cancel its invoked actor. */
        QUIT: "exiting",
      },
    },
    /** Show settings while preserving the loaded Workspace. */
    settings: {
      on: { MODAL_CLOSED: "active.idle" },
    },
    /** Show the load failure and permit only exit. */
    error: {
      on: {
        COMMAND: { guard: ({ event }) => event.code === "app.quit", target: "exiting" },
        QUIT: "exiting",
      },
    },
    /** End the TUI actor. */
    exiting: { type: "final" },
  },
})

/** Build context updates after a Workspace operation completes. */
function workspaceResultContext(result: WorkspaceRunResult): Partial<MachineContext> {
  /** Index each Source result by its stable Source ID. */
  return {
    sourceItems: Object.fromEntries(result.sources.map((source) => [source.id, source.items])),
    sourceRuns: Object.fromEntries(result.sources.map((source) => [source.id, source])),
    runRequest: { mode: "idle", id: 0, stopping: false },
    runError: undefined,
    message: result.cancelled ? "Stopped" : "Ready",
  }
}

/** Build context updates after a Workspace operation fails. */
function workspaceErrorContext(error: unknown): Partial<MachineContext> {
  /** Preserve the runtime failure for the error banner. */
  return {
    runError: String(error),
    runRequest: { mode: "idle", id: 0, stopping: false },
    message: "Run failed",
  }
}

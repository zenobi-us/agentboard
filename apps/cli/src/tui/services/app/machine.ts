import { fromPromise, assign, setup } from "xstate"
import type { LoadedWorkspace } from "../../../services/config/workspace.ts"
import { loadWorkspace } from "../../../services/workspace.ts"

export type AppRoute =
  | { name: "workspace" }
  | { name: "items" }
  | { name: "source"; sourceId: string }
  | { name: "item"; sourceId: string; itemId: string }
  | { name: "action-item"; sourceId: string; itemId: string; actionIndex: number }

type RunRequest = {
  mode: "idle" | "run" | "watch" | "list"
  id: number
  stopping: boolean
}

type ItemRunRequest = {
  sourceId: string
  itemId: string
  id: number
}

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
}

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
  | { type: "RUN_ITEM"; sourceId: string; itemId: string }

export const tuiMachine = setup({
  types: {
    input: {} as { workspacePath: string; version: string },
    context: {} as MachineContext,
    events: {} as MachineEvents,
  },
  actors: {
    loadWorkspace: fromPromise(({ input, signal }: { input: { workspacePath: string }; signal: AbortSignal }) =>
      loadWorkspace(input.workspacePath, undefined, signal, false),
    ),
  },
}).createMachine({
  id: "tui",
  initial: "initialising",
  context: ({ input }) => ({
    message: "Ready",
    route: { name: "workspace" },
    workspacePath: input.workspacePath,
    version: input.version,
    runRequest: { mode: "idle", id: 0, stopping: false },
  }),
  states: {
    initialising: {
      entry: assign({ message: "Initializing...", error: undefined }),
      invoke: {
        src: "loadWorkspace",
        input: ({ context }) => ({ workspacePath: context.workspacePath }),
        onDone: {
          target: "active",
          actions: assign({
            workspaceConfig: ({ event }) => event.output.config,
            executableWorkspace: ({ event }) => event.output,
            message: "Ready",
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
    active: {
      on: {
        COMMAND: [
          {
            guard: ({ event }) => event.code === "app.quit",
            target: "exiting",
          },
          {
            guard: ({ event }) => event.code === "app.refresh",
            actions: assign({ message: "Refreshed" }),
          },
          {
            guard: ({ event }) => event.code === "app.view-workspace",
            actions: assign({ route: { name: "workspace" } }),
          },
          {
            guard: ({ event }) => event.code === "app.view-items",
            actions: assign({ route: { name: "items" } }),
          },
          {
            guard: ({ event, context }) => event.code === "app.run" && context.runRequest.mode === "run",
            actions: assign(({ context }) => ({
              runRequest: { ...context.runRequest, stopping: true },
              message: "Stopping...",
            })),
          },
          {
            guard: ({ event, context }) => event.code === "app.watch" && context.runRequest.mode === "watch",
            actions: assign(({ context }) => ({
              runRequest: { ...context.runRequest, stopping: true },
              message: "Stopping...",
            })),
          },
          {
            guard: ({ event, context }) => event.code === "app.list" && context.runRequest.mode === "list",
            actions: assign(({ context }) => ({
              runRequest: { ...context.runRequest, stopping: true },
              message: "Stopping...",
            })),
          },
          {
            guard: ({ event, context }) => event.code === "app.run" && context.runRequest.mode === "idle",
            actions: assign(({ context }) => ({
              message: "Running...",
              runRequest: { mode: "run" as const, id: context.runRequest.id + 1, stopping: false },
            })),
          },
          {
            guard: ({ event, context }) => event.code === "app.watch" && context.runRequest.mode === "idle",
            actions: assign(({ context }) => ({
              message: "Watching...",
              runRequest: { mode: "watch" as const, id: context.runRequest.id + 1, stopping: false },
            })),
          },
          {
            guard: ({ event, context }) => event.code === "app.list" && context.runRequest.mode === "idle",
            actions: assign(({ context }) => ({
              message: "Listing...",
              runRequest: { mode: "list" as const, id: context.runRequest.id + 1, stopping: false },
            })),
          },
          {
            guard: ({ event }) => event.code === "app.open-settings",
            target: "settings",
          },
          {
            guard: ({ event }) => event.code === "app.stop",
            actions: assign(({ context }) => ({
              runRequest: { ...context.runRequest, stopping: true },
              message: "Stopping...",
            })),
          },
          {
            guard: ({ event }) => event.code === "app.run-stopped" || event.code === "app.run-complete",
            actions: assign({ runRequest: { mode: "idle", id: 0, stopping: false }, message: "Ready" }),
          },
          {
            guard: ({ event }) => event.code === "app.run-failed",
            actions: assign({ runRequest: { mode: "idle", id: 0, stopping: false }, message: "Run failed" }),
          },
          {
            guard: ({ event }) => event.code === "item.run-complete",
            actions: assign({ itemRunRequest: undefined, message: "Ready" }),
          },
          {
            guard: ({ event }) => event.code === "item.run-failed",
            actions: assign({ itemRunRequest: undefined, message: "Item run failed" }),
          },
        ],
        ROUTE_WORKSPACE: {
          actions: assign({ route: { name: "workspace" } }),
        },
        ROUTE_ITEMS: {
          actions: assign({ route: { name: "items" } }),
        },
        ROUTE_SOURCE: {
          actions: assign(({ event }) => ({
            route: { name: "source", sourceId: event.sourceId },
          })),
        },
        ROUTE_ITEM: {
          actions: assign(({ event }) => ({
            route: {
              name: "item",
              sourceId: event.sourceId,
              itemId: event.itemId,
            },
          })),
        },
        ROUTE_ACTION_ITEM: {
          actions: assign(({ event }) => ({
            route: {
              name: "action-item",
              sourceId: event.sourceId,
              itemId: event.itemId,
              actionIndex: event.actionIndex,
            },
          })),
        },
        REFRESH: {
          actions: assign({ message: "Refreshed" }),
        },
        RUN_ITEM: {
          actions: assign(({ context, event }) => ({
            itemRunRequest: {
              sourceId: event.sourceId,
              itemId: event.itemId,
              id: (context.itemRunRequest?.id ?? 0) + 1,
            },
            message: "Running item...",
          })),
        },
        QUIT: "exiting",
      },
    },
    settings: {
      on: {
        MODAL_CLOSED: "active",
      },
    },
    error: {
      on: {
        COMMAND: {
          guard: ({ event }) => event.code === "app.quit",
          target: "exiting",
        },
        QUIT: "exiting",
      },
    },
    exiting: {
      type: "final",
    },
  },
})

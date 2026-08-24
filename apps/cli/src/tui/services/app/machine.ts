import { fromPromise, assign, setup } from "xstate"
import type { LoadedWorkspace } from "../../../services/config/workspace.ts"
import { loadWorkspace } from "../../../services/workspace.ts"

export type AppRoute =
  | { name: "workspace" }
  | { name: "source"; sourceId: string }
  | { name: "item"; sourceId: string; itemId: string }

type RunRequest = {
  mode: "idle" | "run" | "watch" | "list"
  id: number
  stopping: boolean
}

type MachineContext = {
  message: string
  route: AppRoute
  workspacePath: string
  version: string
  runRequest: RunRequest
  workspaceConfig?: unknown
  executableWorkspace?: LoadedWorkspace
  error?: string
}

type MachineEvents =
  | { type: "COMMAND"; code: string }
  | { type: "ROUTE_WORKSPACE" }
  | { type: "ROUTE_SOURCE"; sourceId: string }
  | { type: "ROUTE_ITEM"; sourceId: string; itemId: string }
  | { type: "REFRESH" }
  | { type: "QUIT" }
  | { type: "MODAL_CLOSED" }

export const tuiMachine = setup({
  types: {
    input: {} as { workspacePath: string; version: string },
    context: {} as MachineContext,
    events: {} as MachineEvents,
  },
  actors: {
    loadWorkspace: fromPromise(({ input, signal }: { input: { workspacePath: string }; signal: AbortSignal }) =>
      loadWorkspace(input.workspacePath, undefined, signal, true),
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
        ],
        ROUTE_WORKSPACE: {
          actions: assign({ route: { name: "workspace" } }),
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
        REFRESH: {
          actions: assign({ message: "Refreshed" }),
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

import { assign, setup } from "xstate"

export type AppRoute =
  | { name: "workspace" }
  | { name: "source"; sourceId: string }
  | { name: "item"; sourceId: string; itemId: string }

type MachineContext = {
  message: string
  route: AppRoute
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
    context: {} as MachineContext,
    events: {} as MachineEvents,
  },
}).createMachine({
  id: "tui",
  initial: "initialising",
  context: {
    message: "Ready",
    route: { name: "workspace" },
  },
  states: {
    initialising: {
      entry: assign({ message: "Initializing..." }),
      after: {
        5000: "active",
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
            guard: ({ event }) => event.code === "app.open-settings",
            target: "settings",
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
    exiting: {
      type: "final",
    },
  },
})

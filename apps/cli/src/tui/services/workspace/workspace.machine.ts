import type { AnyActorRef } from "xstate"
import { assign, setup } from "xstate"

type WorkspaceContext = {
  appActor: AnyActorRef
  sourceIndex: number
  itemIndex: number
  sources: readonly string[]
  items: readonly string[]
}

type WorkspaceEvent =
  | { type: "COMMAND"; code: string }
  | { type: "SOURCE_SELECTED"; index: number }
  | { type: "ITEM_SELECTED"; index: number }

export const workspaceMachine = setup({
  types: {
    input: {} as { appActor: AnyActorRef; sources: readonly string[] },
    context: {} as WorkspaceContext,
    events: {} as WorkspaceEvent,
  },
}).createMachine({
  id: "workspace",
  initial: "ready",
  context: ({ input }) => ({
    appActor: input.appActor,
    sourceIndex: 0,
    itemIndex: 0,
    sources: input.sources,
    items: ["Fix sync failure", "Review source mapping", "Update dashboard"],
  }),
  states: {
    ready: {
      on: {
        COMMAND: [
          {
            guard: ({ event }) => event.code === "workspace.next",
            actions: assign({
              sourceIndex: ({ context }) => (context.sourceIndex + 1) % context.sources.length,
            }),
          },
          {
            guard: ({ event }) => event.code === "workspace.previous",
            actions: assign({
              sourceIndex: ({ context }) =>
                (context.sourceIndex - 1 + context.sources.length) % context.sources.length,
            }),
          },
          {
            guard: ({ event }) => event.code === "workspace.open-source",
            actions: ({ context }) =>
              context.appActor.send({
                type: "ROUTE_SOURCE",
                sourceId: context.sources[context.sourceIndex],
              }),
          },
          {
            guard: ({ event }) => event.code === "workspace.open-item",
            actions: ({ context }) =>
              context.appActor.send({
                type: "ROUTE_ITEM",
                sourceId: context.sources[context.sourceIndex],
                itemId: context.items[context.itemIndex],
              }),
          },
          {
            guard: ({ event }) => event.code === "workspace.refresh",
            actions: assign({ itemIndex: 0 }),
          },
        ],
        SOURCE_SELECTED: {
          actions: assign({ sourceIndex: ({ event }) => event.index }),
        },
        ITEM_SELECTED: {
          actions: assign({ itemIndex: ({ event }) => event.index }),
        },
      },
    },
  },
})

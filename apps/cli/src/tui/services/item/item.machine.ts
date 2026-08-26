import type { AnyActorRef } from "xstate"
import { setup } from "xstate"

type ItemContext = {
  appActor: AnyActorRef
  sourceId: string
  itemId: string
}

type ItemEvent = { type: "COMMAND"; code: string }

export const itemMachine = setup({
  types: {
    input: {} as { appActor: AnyActorRef; sourceId: string; itemId: string },
    context: {} as ItemContext,
    events: {} as ItemEvent,
  },
}).createMachine({
  id: "item",
  initial: "ready",
  context: ({ input }) => input,
  states: {
    ready: {
      on: {
        COMMAND: [
          {
            guard: ({ event }) => event.code === "item.back",
            actions: ({ context }) =>
              context.appActor.send({
                type: "ROUTE_SOURCE",
                sourceId: context.sourceId,
              }),
          },
          {
            guard: ({ event }) => event.code === "item.run",
            actions: ({ context }) =>
              context.appActor.send({
                type: "RUN_ITEM",
                sourceId: context.sourceId,
                itemId: context.itemId,
              }),
          },
        ],
      },
    },
  },
})

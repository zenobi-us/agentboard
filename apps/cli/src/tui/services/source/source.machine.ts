import type { AnyActorRef } from "xstate"
import { assign, setup } from "xstate"

export type SourceMachineInput<TItem> = {
  appActor: AnyActorRef
  sourceId: string
  items: readonly TItem[]
  getItemId: (item: TItem) => string
}

type SourceMachineContext<TItem> = {
  appActor: AnyActorRef
  sourceId: string
  itemIndex: number
  items: readonly TItem[]
  getItemId: (item: TItem) => string
}

type SourceEvent = { type: "COMMAND"; code: string }

export function createSourceMachine<TItem>() {
  return setup({
    types: {
      input: {} as SourceMachineInput<TItem>,
      context: {} as SourceMachineContext<TItem>,
      events: {} as SourceEvent,
    },
  }).createMachine({
    id: "source",
    initial: "ready",
    context: ({ input }) => ({
      appActor: input.appActor,
      sourceId: input.sourceId,
      itemIndex: 0,
      items: input.items,
      getItemId: input.getItemId,
    }),
    states: {
      ready: {
        on: {
          COMMAND: [
            {
              guard: ({ event }) => event.code === "source.back",
              actions: ({ context }) => context.appActor.send({ type: "ROUTE_WORKSPACE" }),
            },
            {
              guard: ({ event, context }) => event.code === "source.next" && context.items.length > 0,
              actions: assign({
                itemIndex: ({ context }) => (context.itemIndex + 1) % context.items.length,
              }),
            },
            {
              guard: ({ event, context }) => event.code === "source.previous" && context.items.length > 0,
              actions: assign({
                itemIndex: ({ context }) =>
                  (context.itemIndex - 1 + context.items.length) % context.items.length,
              }),
            },
            {
              guard: ({ event }) => event.code === "source.open-item",
              actions: ({ context }) => {
                const item = context.items[context.itemIndex]
                if (!item) return
                context.appActor.send({
                  type: "ROUTE_ITEM",
                  sourceId: context.sourceId,
                  itemId: context.getItemId(item),
                })
              },
            },
          ],
        },
      },
    },
  })
}

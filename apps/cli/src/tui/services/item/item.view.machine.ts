import type { Item } from "@agentboard/core/config"
import { fromPromise, assign, sendParent, setup } from "xstate"
import type { AnyActorRef } from "xstate"
import type { LoadedWorkspace } from "../../../services/config/workspace.ts"
import { runItem, type SourceRunResult } from "../../../services/runtime.ts"

type ItemViewContext = {
  appActor: AnyActorRef
  sourceId: string
  itemId: string
  item: Item
}

type ItemViewEvent = { type: "COMMAND"; code: string }

export const itemViewMachine = setup({
  types: {
    input: {} as { appActor: AnyActorRef; sourceId: string; itemId: string; item: Item },
    context: {} as ItemViewContext,
    events: {} as ItemViewEvent,
  },
}).createMachine({
  id: "itemView",
  initial: "ready",
  context: ({ input }) => input,
  states: {
    ready: {
      on: {
        COMMAND: [
          {
            guard: ({ event }) => event.code === "item.back",
            actions: ({ context }) => context.appActor.send({ type: "ROUTE_SOURCE", sourceId: context.sourceId }),
          },
          {
            guard: ({ event }) => event.code === "item.run",
            actions: ({ context }) => context.appActor.send({
              type: "RUN_ITEM",
              sourceId: context.sourceId,
              itemId: context.itemId,
              item: context.item,
            }),
          },
        ],
      },
    },
  },
})

type ItemRunRequest = {
  sourceId: string
  item: Item
}

export const itemRunMachine = setup({
  types: {
    input: {} as { workspace: LoadedWorkspace },
    context: {} as { workspace: LoadedWorkspace; request?: ItemRunRequest },
    events: {} as { type: "RUN_ITEM"; sourceId: string; item: Item },
  },
  actors: {
    runItem: fromPromise(async ({ input, signal }: {
      input: { workspace: LoadedWorkspace; request: ItemRunRequest }
      signal: AbortSignal
    }): Promise<SourceRunResult> => {
      const workspace = {
        ...input.workspace,
        cancellation: signal,
        sources: input.workspace.sources.map((source) => ({ ...source, cancellation: signal })),
      }
      return runItem(workspace, input.request.sourceId, input.request.item)
    }),
  },
}).createMachine({
  id: "itemRun",
  initial: "idle",
  context: ({ input }) => ({ workspace: input.workspace }),
  states: {
    idle: {
      on: {
        RUN_ITEM: {
          target: "running",
          actions: assign({
            request: ({ event }) => ({ sourceId: event.sourceId, item: event.item }),
          }),
        },
      },
    },
    running: {
      invoke: {
        src: "runItem",
        input: ({ context }) => ({ workspace: context.workspace, request: context.request! }),
        onDone: {
          target: "idle",
          actions: sendParent(({ event }) => ({ type: "ITEM_RUN_COMPLETE", result: event.output })),
        },
        onError: {
          target: "idle",
          actions: sendParent(({ event }) => ({ type: "ITEM_RUN_FAILED", error: String(event.error) })),
        },
      },
    },
  },
})

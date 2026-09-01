import type { Item } from "@clankpipe/core/config"
import { fromPromise, assign, sendParent, setup } from "xstate"
import type { AnyActorRef } from "xstate"
import type { LoadedWorkspace } from "../../../services/config/workspace.ts"
import { openItem } from "../../../services/open-item.ts"
import { runItem, type SourceRunResult } from "../../../services/runtime.ts"

type ItemViewContext = {
  appActor: AnyActorRef
  sourceId: string
  itemId: string
  item: Item
  openCommand?: string
  templateContext?: Record<string, unknown>
  openError?: string
}

type ItemViewEvent = { type: "COMMAND"; code: string }

export const itemViewMachine = setup({
  types: {
    input: {} as {
      appActor: AnyActorRef
      sourceId: string
      itemId: string
      item: Item
      openCommand?: string
      templateContext?: Record<string, unknown>
    },
    context: {} as ItemViewContext,
    events: {} as ItemViewEvent,
  },
  actors: {
    openItem: fromPromise(async ({ input, signal }: {
      input: { command: string; context: Record<string, unknown> }
      signal: AbortSignal
    }) => openItem(input.command, input.context, signal)),
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
          {
            guard: ({ event, context }) => event.code === "item.open" && context.openCommand !== undefined,
            target: "opening",
            actions: assign({ openError: undefined }),
          },
          {
            guard: ({ event }) => event.code === "item.open",
            actions: assign({ openError: "No open command configured." }),
          },
        ],
      },
    },
    opening: {
      invoke: {
        src: "openItem",
        input: ({ context }) => ({
          command: context.openCommand!,
          context: context.templateContext ?? {},
        }),
        onDone: "ready",
        onError: {
          target: "ready",
          actions: assign({ openError: ({ event }) => String(event.error) }),
        },
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

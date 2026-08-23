import { setup } from "xstate"

export const modalMachine = setup({
  types: {
    events: {} as
      | { type: "COMMAND"; code: "modal.close" }
      | { type: "COMMAND"; code: "settings.save" },
  },
}).createMachine({
  id: "settings-modal",
  initial: "open",
  states: {
    open: {
      on: {
        COMMAND: [
          {
            guard: ({ event }) => event.code === "modal.close",
            target: "closed",
          },
        ],
      },
    },
    closed: {
      type: "final",
    },
  },
})

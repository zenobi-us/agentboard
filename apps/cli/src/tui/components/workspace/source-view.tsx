import { useActorRef, useSelector } from "@xstate/react"
import type { AnyActorRef } from "xstate"
import { KeymapScope, sourceKeymap } from "../../services/keymaps.tsx"
import { createSourceMachine, type SourceMachineInput } from "../../services/source/source.machine.ts"

type SourceViewProps<TItem> = {
  appActor: AnyActorRef
  sourceId: string
  items: readonly TItem[]
  getItemId: (item: TItem) => string
  getItemLabel: (item: TItem) => string
}

export function SourceView<TItem>(props: SourceViewProps<TItem>) {
  const machine = createSourceMachine<TItem>()
  const input: SourceMachineInput<TItem> = {
    appActor: props.appActor,
    sourceId: props.sourceId,
    items: props.items,
    getItemId: props.getItemId,
  }
  const actor = useActorRef(machine, { input })
  const snapshot = useSelector(actor, (value) => value)

  return (
    <KeymapScope actor={actor} bindings={sourceKeymap}>
      <box flexDirection="column" flexGrow={1}>
        <text fg="#f2c94c">SOURCE / {snapshot.context.sourceId}</text>
        <text marginTop={1}>Use Up and Down to select an item.</text>
        <text>Press Return to open the item.</text>
        <text>Press Escape to return to the workspace.</text>
        <box flexDirection="column" marginTop={1}>
          {snapshot.context.items.map((item, index) => (
            <text
              key={props.getItemId(item)}
              fg={index === snapshot.context.itemIndex ? "#f2c94c" : "#d8dee9"}
            >
              {index === snapshot.context.itemIndex ? "> " : "  "}{props.getItemLabel(item)}
            </text>
          ))}
        </box>
      </box>
    </KeymapScope>
  )
}

export type DemoSourceItem = { id: string; title: string }
export const demoSourceMachine = createSourceMachine<DemoSourceItem>()

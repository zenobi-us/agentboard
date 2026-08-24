import { useActorRef, useSelector } from "@xstate/react"
import type { Item } from "@agentboard/core/config"
import type { AnyActorRef } from "xstate"
import { KeymapScope, sourceKeymap } from "../../services/keymaps.tsx"
import { createSourceMachine, type SourceMachineInput } from "../../services/source/source.machine.ts"
import { useTheme } from "../../services/theme/theme.tsx"

const sourceMachine = createSourceMachine<Item>()

type SourceViewProps = {
  appActor: AnyActorRef
  sourceId: string
  items: readonly Item[]
}

export function SourceView(props: SourceViewProps) {
  const input: SourceMachineInput<Item> = {
    appActor: props.appActor,
    sourceId: props.sourceId,
    items: props.items,
    getItemId: (item) => item.id,
  }
  const actor = useActorRef(sourceMachine, { input })
  const snapshot = useSelector(actor, (value) => value)
  const theme = useTheme()
  const headingStyle = theme.component("source.heading")
  const itemStyle = theme.component("source.item")
  const selectedStyle = theme.component("source.item.selected")

  return (
    <KeymapScope actor={actor} bindings={sourceKeymap}>
      <box flexDirection="column" flexGrow={1}>
        <text fg={headingStyle.fg}>SOURCE / {snapshot.context.sourceId}</text>
        <text marginTop={1}>Use Up and Down to select an item.</text>
        <text>Press Return to open the item.</text>
        <text>Press Escape to return to the workspace.</text>
        <box flexDirection="column" marginTop={1}>
          {snapshot.context.items.map((item, index) => (
            <text
              key={item.id}
              fg={(index === snapshot.context.itemIndex ? selectedStyle : itemStyle).fg}
            >
              {index === snapshot.context.itemIndex ? "> " : "  "}{item.title} · {item.status}
            </text>
          ))}
        </box>
      </box>
    </KeymapScope>
  )
}

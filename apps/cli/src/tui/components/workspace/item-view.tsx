import { useActorRef, useSelector } from "@xstate/react"
import type { AnyActorRef } from "xstate"
import { KeymapScope, itemKeymap } from "../../services/keymaps.tsx"
import { itemMachine } from "../../services/item/item.machine.ts"
import { useTheme } from "../../services/theme/theme.tsx"

export function ItemView(props: {
  appActor: AnyActorRef
  sourceId: string
  itemId: string
}) {
  const actor = useActorRef(itemMachine, {
    input: {
      appActor: props.appActor,
      sourceId: props.sourceId,
      itemId: props.itemId,
    },
  })
  const snapshot = useSelector(actor, (value) => value)
  const headingStyle = useTheme().component("item.heading")

  return (
    <KeymapScope actor={actor} bindings={itemKeymap}>
      <box flexDirection="column" flexGrow={1}>
        <text fg={headingStyle.fg}>ITEM</text>
        <text marginTop={1}>Source: {snapshot.context.sourceId}</text>
        <text>Title: {snapshot.context.itemId}</text>
        <text marginTop={1}>Press R to run the item action.</text>
        <text>Press Escape to return to the source.</text>
      </box>
    </KeymapScope>
  )
}

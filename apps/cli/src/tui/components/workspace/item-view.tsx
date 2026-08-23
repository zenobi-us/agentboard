import { useActorRef, useSelector } from "@xstate/react"
import type { AnyActorRef } from "xstate"
import { KeymapScope, itemKeymap } from "../../services/keymaps.tsx"
import { itemMachine } from "../../services/item/item.machine.ts"

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

  return (
    <KeymapScope actor={actor} bindings={itemKeymap}>
      <box flexDirection="column" flexGrow={1}>
        <text fg="#f2c94c">ITEM</text>
        <text marginTop={1}>Source: {snapshot.context.sourceId}</text>
        <text>Title: {snapshot.context.itemId}</text>
        <text marginTop={1}>Press R to run the item action.</text>
        <text>Press Escape to return to the source.</text>
      </box>
    </KeymapScope>
  )
}

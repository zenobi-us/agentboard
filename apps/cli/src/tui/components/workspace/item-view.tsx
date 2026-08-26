import { useActorRef, useSelector } from "@xstate/react"
import type { AnyActorRef } from "xstate"
import type { SourceRunResult } from "../../../services/runtime.ts"
import { KeymapScope, itemKeymap } from "../../services/keymaps.tsx"
import { itemMachine } from "../../services/item/item.machine.ts"
import { useTheme } from "../../services/theme/theme.tsx"

export function ItemView(props: {
  appActor: AnyActorRef
  sourceId: string
  itemId: string
  runResult?: SourceRunResult
  running?: boolean
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
        <text marginTop={1}>{props.running ? "Running item actions..." : "Press R to run the item actions."}</text>
        {props.runResult?.actions
          .filter((result) => result.itemId === props.itemId)
          .map((result) => (
            <box key={`${result.itemId}:${result.actionIndex}`} flexDirection="column" marginTop={1}>
              <text>Action {result.actionIndex + 1}: {result.result?.outcome ?? result.error ?? (result.skipped ? "skipped" : "pending")}</text>
              <text>Stdout: {result.result?.stdout || "(none)"}</text>
              <text>Stderr: {result.result?.stderr || "(none)"}</text>
            </box>
          ))}
        <text>Press Escape to return to the source.</text>
      </box>
    </KeymapScope>
  )
}

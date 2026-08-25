import { useActorRef } from "@xstate/react"
import type { Item } from "@agentboard/core/config"
import type { AnyActorRef } from "xstate"
import type { LoadedWorkspaceSource } from "../../../services/config/workspace.ts"
import type { SourceRunResult } from "../../../services/runtime.ts"
import { KeymapScope, itemKeymap } from "../../services/keymaps.tsx"
import { itemMachine } from "../../services/item/item.machine.ts"
import { useTheme } from "../../services/theme/theme.tsx"

export function ActionItemView(props: {
  appActor: AnyActorRef
  source: LoadedWorkspaceSource
  item: Item
  actionIndex: number
  runResult?: SourceRunResult
}) {
  const actor = useActorRef(itemMachine, {
    input: {
      appActor: props.appActor,
      sourceId: props.source.id,
      itemId: props.item.id,
    },
  })
  const headingStyle = useTheme().component("item.heading")
  const action = props.source.actions[props.actionIndex]
  const result = props.runResult?.actions.find(
    (candidate) => candidate.itemId === props.item.id && candidate.actionIndex === props.actionIndex,
  )

  if (!action) return null

  return (
    <KeymapScope actor={actor} bindings={itemKeymap}>
      <box flexDirection="column" flexGrow={1}>
        <text fg={headingStyle.fg}>ACTION ITEM</text>
        <text marginTop={1}>Source: {props.source.id}</text>
        <text>Reference: {props.item.reference_id}</text>
        <text>Title: {props.item.title}</text>
        <text>Status: {props.item.status}</text>
        <text>Action: {action.id ?? action.packageName}</text>
        <text>Step: {props.actionIndex + 1}</text>
        <text>Uses: {action.packageName}</text>
        <text>Outcome: {actionOutcome(result)}</text>
        <text marginTop={1}>Action configuration:</text>
        <text>{JSON.stringify(action.config, null, 2)}</text>
        <text marginTop={1}>Stdout:</text>
        <text>{result?.result?.stdout || "(none)"}</text>
        <text marginTop={1}>Stderr:</text>
        <text>{result?.result?.stderr || "(none)"}</text>
        {result?.result?.message || result?.error ? (
          <>
            <text marginTop={1}>Message:</text>
            <text>{result.result?.message ?? result.error}</text>
          </>
        ) : null}
        <text marginTop={1}>Press Escape to return to the source.</text>
      </box>
    </KeymapScope>
  )
}

function actionOutcome(result: SourceRunResult["actions"][number] | undefined): string {
  if (!result) return "pending"
  if (result.error) return "error"
  if (result.skipped) return "skipped"
  return result.result?.outcome ?? "pending"
}

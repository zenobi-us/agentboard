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
  const theme = useTheme()
  const headingStyle = theme.component("item.heading")
  const panelStyle = theme.component("source.summary")
  const action = props.source.actions[props.actionIndex]
  const result = props.runResult?.actions.find(
    (candidate) => candidate.itemId === props.item.id && candidate.actionIndex === props.actionIndex,
  )

  if (!action) return null

  return (
    <KeymapScope actor={actor} bindings={itemKeymap}>
      <box flexDirection="column" flexGrow={1}>
        <text fg={headingStyle.fg}>ACTION / {props.source.id} / {props.item.reference_id} / STEP {props.actionIndex + 1}</text>

        <box
          border={true}
          borderStyle="single"
          borderColor={panelStyle.border}
          padding={1}
          marginTop={1}
          marginBottom={1}
          flexDirection="column"
        >
          <text fg={panelStyle.fg}>{props.item.title}</text>
          <box flexDirection="row" marginTop={1}>
            <box flexDirection="column" marginRight={3}>
              <text>Source  {props.source.id}</text>
              <text>Status  {props.item.status}</text>
            </box>
            <box flexDirection="column">
              <text>Reference  {props.item.reference_id}</text>
              <text>Action     {action.id ?? action.packageName}</text>
            </box>
          </box>
        </box>

        <box
          border={true}
          borderStyle="single"
          borderColor={panelStyle.border}
          padding={1}
          marginBottom={1}
          flexDirection="column"
        >
          <text fg={panelStyle.fg}>EXECUTION</text>
          <text marginTop={1}>Step     {props.actionIndex + 1}</text>
          <text>Uses     {action.packageName}</text>
          <text>Outcome  {actionOutcome(result)}</text>
        </box>

        <box
          border={true}
          borderStyle="single"
          borderColor={panelStyle.border}
          padding={1}
          marginBottom={1}
          flexDirection="column"
        >
          <text fg={panelStyle.fg}>OUTPUT</text>
          <text marginTop={1}>Stdout</text>
          <text>{result?.result?.stdout || "(none)"}</text>
          <text marginTop={1}>Stderr</text>
          <text>{result?.result?.stderr || "(none)"}</text>
          {result?.result?.message || result?.error ? (
            <>
              <text marginTop={1}>Message</text>
              <text>{result.result?.message ?? result.error}</text>
            </>
          ) : null}
        </box>

        <box
          border={true}
          borderStyle="single"
          borderColor={panelStyle.border}
          padding={1}
          marginBottom={1}
          flexDirection="column"
        >
          <text fg={panelStyle.fg}>CONFIGURATION</text>
          <text marginTop={1}>{JSON.stringify(action.config, null, 2)}</text>
        </box>

        <text>Press Escape to return to the source.</text>
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

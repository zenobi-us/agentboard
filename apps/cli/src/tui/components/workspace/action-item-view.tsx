import { useActorRef, useSelector } from "@xstate/react"

import type { SourceRunResult } from "../../../services/runtime.ts"
import { KeymapScope, itemKeymap } from "../../services/keymaps.tsx"
import { itemViewMachine } from "../../services/item/item.view.machine.ts"
import { useTheme } from "../../services/theme/theme.tsx"
import { useAppMachine } from "../../services/app/provider.tsx"

/** Render one Action result using data selected from the root machine. */
export function ActionItemView() {
  /** Reference the root machine actor. */
  const appActor = useAppMachine()
  /** Read the active Action route. */
  const route = useSelector(appActor, (snapshot) => snapshot.context.route)
  /** Read the loaded Workspace. */
  const workspace = useSelector(appActor, (snapshot) => snapshot.context.executableWorkspace)
  /** Read Source items from machine context. */
  const sourceItems = useSelector(appActor, (snapshot) => snapshot.context.sourceItems)
  /** Read Source run results from machine context. */
  const sourceRuns = useSelector(appActor, (snapshot) => snapshot.context.sourceRuns)
  if (route.name !== "action-item" || !workspace) return null
  /** Resolve the Action Source. */
  const source = workspace.sources.find((candidate) => candidate.id === route.sourceId)
  /** Resolve the Action Item. */
  const item = sourceItems[route.sourceId]?.find((candidate) => candidate.id === route.itemId)
  if (!source || !item) return null
  /** Resolve the latest Source run result. */
  const runResult = sourceRuns[source.id]
  /** Create the Item navigation actor. */
  const actor = useActorRef(itemViewMachine, {
    input: {
      appActor,
      sourceId: source.id,
      itemId: item.id,
      item,
    },
  })
  const theme = useTheme()
  const headingStyle = theme.component("item.heading")
  const panelStyle = theme.component("source.summary")
  /** Resolve the selected Action. */
  const action = source.actions[route.actionIndex]
  const result = runResult?.actions.find(
    (candidate) => candidate.itemId === item.id && candidate.actionIndex === route.actionIndex,
  )

  if (!action) return null

  return (
    <KeymapScope actor={actor} bindings={itemKeymap}>
      <box flexDirection="column" flexGrow={1}>
        <text fg={headingStyle.fg}>ACTION / {source.id} / {item.reference_id} / STEP {route.actionIndex + 1}</text>

        <box
          border={true}
          borderStyle="single"
          borderColor={panelStyle.border}
          padding={1}
          marginTop={1}
          marginBottom={1}
          flexDirection="column"
        >
          <text fg={panelStyle.fg}>{item.title}</text>
          <box flexDirection="row" marginTop={1}>
            <box flexDirection="column" marginRight={3}>
              <text>Source  {source.id}</text>
              <text>Status  {item.status}</text>
            </box>
            <box flexDirection="column">
              <text>Reference  {item.reference_id}</text>
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
          <text marginTop={1}>Step     {route.actionIndex + 1}</text>
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

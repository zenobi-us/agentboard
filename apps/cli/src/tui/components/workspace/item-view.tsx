import { useActorRef, useSelector } from "@xstate/react"
import type { Item } from "@agentboard/core/config"
import type { AnyActorRef } from "xstate"
import type { LoadedWorkspaceSource } from "../../../services/config/workspace.ts"
import type { SourceRunResult } from "../../../services/runtime.ts"
import { KeymapScope, itemKeymap } from "../../services/keymaps.tsx"
import { itemViewMachine } from "../../services/item/item.view.machine.ts"
import { useTheme } from "../../services/theme/theme.tsx"

export function ItemView(props: {
  appActor: AnyActorRef
  source: LoadedWorkspaceSource
  item: Item
  runResult?: SourceRunResult
  running?: boolean
}) {
  const actor = useActorRef(itemViewMachine, {
    input: {
      appActor: props.appActor,
      sourceId: props.source.id,
      itemId: props.item.id,
      item: props.item,
    },
  })
  const snapshot = useSelector(actor, (value) => value)
  const theme = useTheme()
  const headingStyle = theme.component("item.heading")
  const panelStyle = theme.component("source.summary")
  const results = props.source.actions.map((action, actionIndex) => ({
    action,
    actionIndex,
    result: props.runResult?.actions.find(
      (candidate) => candidate.itemId === props.item.id && candidate.actionIndex === actionIndex,
    ),
  }))
  const completed = results.filter(({ result }) => result?.result || result?.error || result?.skipped).length
  const output = latestOutput(results)

  return (
    <KeymapScope actor={actor} bindings={itemKeymap}>
      <box flexDirection="column" flexGrow={1}>
        <text fg={headingStyle.fg}>ITEM / {snapshot.context.sourceId} / {props.item.reference_id}</text>

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
              <text>URL        {props.item.url || "(none)"}</text>
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
          <text fg={panelStyle.fg}>ACTION PIPELINE</text>
          {results.length === 0 ? <text marginTop={1}>No actions configured.</text> : null}
          {results.map(({ action, actionIndex, result }) => (
            <text
              key={`${action.packageName}:${actionIndex}`}
              marginTop={1}
              onMouseDown={() => props.appActor.send({
                type: "ROUTE_ACTION_ITEM",
                sourceId: props.source.id,
                itemId: props.item.id,
                actionIndex,
              })}
            >
              {statusSymbol(result, props.running === true && !result)}  {action.id ?? action.packageName} · {actionOutcome(result, props.running === true && !result)}
            </text>
          ))}
        </box>

        <box flexDirection="column" marginBottom={1}>
          <text fg={panelStyle.fg}>LAST RUN</text>
          <text marginTop={1}>
            {props.running ? "Running actions..." : props.runResult ? `${completed} of ${results.length} actions completed` : "No run recorded."}
          </text>
          {output ? <text>Output  {output}</text> : null}
        </box>

        <text>{props.running ? "Actions are running..." : "Press R to run actions."}  Press Escape to return.</text>
      </box>
    </KeymapScope>
  )
}

function actionOutcome(
  result: SourceRunResult["actions"][number] | undefined,
  running: boolean,
): string {
  if (running) return "running"
  if (!result) return "pending"
  if (result.error) return "error"
  if (result.skipped) return "skipped"
  return result.result?.outcome ?? "pending"
}

function statusSymbol(result: SourceRunResult["actions"][number] | undefined, running: boolean): string {
  if (running) return "…"
  if (!result) return "○"
  if (result.error || result.result?.outcome === "failure") return "×"
  if (result.skipped) return "–"
  if (result.result?.outcome === "success") return "●"
  return "○"
}

function latestOutput(
  results: readonly { result: SourceRunResult["actions"][number] | undefined }[],
): string | undefined {
  const output = [...results]
    .reverse()
    .map(({ result }) => result?.result?.stdout?.trim())
    .find((value): value is string => Boolean(value))
  return output?.replace(/\s+/g, " ")
}

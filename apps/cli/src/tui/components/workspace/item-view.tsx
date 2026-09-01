import { useActorRef, useSelector } from "@xstate/react"
import type { SourceRunResult } from "../../../services/runtime.ts"
import { KeymapScope, itemKeymap } from "../../services/keymaps.tsx"
import { itemViewMachine } from "../../services/item/item.view.machine.ts"
import { useTheme } from "../../services/theme/theme.tsx"
import { useAppMachine } from "../../services/app/provider.tsx"
import { Breadcrumbs } from "../app/breadcrumbs.tsx"
import { Badge } from "../app/badge.tsx"
import { DefinitionGrid, DefinitionGridItem } from "../app/definition-grid.tsx"

/** Render one Item using data selected from the root machine. */
export function ItemView() {
  /** Reference the root machine actor. */
  const appActor = useAppMachine()
  /** Read the active Item route. */
  const route = useSelector(appActor, (snapshot) => snapshot.context.route)
  /** Read the loaded Workspace. */
  const workspace = useSelector(appActor, (snapshot) => snapshot.context.executableWorkspace)
  /** Read Source items from machine context. */
  const sourceItems = useSelector(appActor, (snapshot) => snapshot.context.sourceItems)
  /** Read Source run results from machine context. */
  const sourceRuns = useSelector(appActor, (snapshot) => snapshot.context.sourceRuns)
  /** Read the active Item operation request. */
  const itemRunRequest = useSelector(appActor, (snapshot) => snapshot.context.itemRunRequest)
  if (route.name !== "item" || !workspace) return null
  /** Resolve the Item Source. */
  const source = workspace.sources.find((candidate) => candidate.id === route.sourceId)
  /** Resolve the Item data. */
  const item = sourceItems[route.sourceId]?.find((candidate) => candidate.id === route.itemId)
  if (!source || !item) return null
  /** Resolve the latest Source run result. */
  const runResult = sourceRuns[source.id]
  /** Mark the selected Item as running. */
  const running = itemRunRequest?.sourceId === source.id && itemRunRequest.itemId === item.id
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
  const panelStyle = theme.component("source.summary")
  const workspaceId = workspace.id
  const results = source.actions.map((action, actionIndex) => ({
    action,
    actionIndex,
    result: runResult?.actions.find(
      (candidate) => candidate.itemId === item.id && candidate.actionIndex === actionIndex,
    ),
  }))
  const completed = results.filter(({ result }) => result?.result || result?.error || result?.skipped).length
  const output = latestOutput(results)

  return (
    <KeymapScope actor={actor} bindings={itemKeymap}>
      <box flexDirection="column" flexGrow={1}>
        <Breadcrumbs.Row>
          <Breadcrumbs.Item onClick={() => appActor.send({ type: "ROUTE_WORKSPACE" })}>
            <Badge.Type type="Workspace" label={workspaceId} />
          </Breadcrumbs.Item>
          <Breadcrumbs.Separator />
          <Breadcrumbs.Item onClick={() => appActor.send({ type: "ROUTE_SOURCE", sourceId: source.id })}>
            <Badge.Type type="Source" label={source.id} />
          </Breadcrumbs.Item>
          <Breadcrumbs.Separator />
          <Breadcrumbs.Item>
            <Badge.Type type="Item" label={item.reference_id} />
          </Breadcrumbs.Item>
        </Breadcrumbs.Row>

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
          <box marginTop={1}>
            <DefinitionGrid>
              <DefinitionGridItem label="Source">{source.id}</DefinitionGridItem>
              <DefinitionGridItem label="Status">{item.status}</DefinitionGridItem>
              <DefinitionGridItem label="Reference">{item.reference_id}</DefinitionGridItem>
              <DefinitionGridItem label="URL">{item.url || "(none)"}</DefinitionGridItem>
            </DefinitionGrid>
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
              onMouseDown={() => appActor.send({
                type: "ROUTE_ACTION_ITEM",
                sourceId: source.id,
                itemId: item.id,
                actionIndex,
              })}
            >
              {statusSymbol(result, running === true && !result)}  {action.id ?? action.packageName} · {actionOutcome(result, running === true && !result)}
            </text>
          ))}
        </box>

        <box flexDirection="column" marginBottom={1}>
          <text fg={panelStyle.fg}>LAST RUN</text>
          <text marginTop={1}>
            {running ? "Running actions..." : runResult ? `${completed} of ${results.length} actions completed` : "No run recorded."}
          </text>
          {output ? <text>Output  {output}</text> : null}
        </box>

        <text>{running ? "Actions are running..." : "Press R to run actions."}  Press Escape to return.</text>
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

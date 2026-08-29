import { useActorRef, useSelector } from "@xstate/react"
import type { Item } from "@agentboard/core/config"
import type { AnyActorRef } from "xstate"
import { KeymapScope, sourceKeymap } from "../../services/keymaps.tsx"
import { createSourceMachine, type SourceMachineInput } from "../../services/source/source.machine.ts"
import { useTheme } from "../../services/theme/theme.tsx"
import { SourceSummaryCard } from "./source-summary-card.tsx"
import type { LoadedWorkspaceSource } from "../../../services/config/workspace.ts"
import type { ActionRunResult } from "../../../services/runtime.ts"
import { Breadcrumbs } from "../app/breadcrumbs.tsx"
import { useAppMachine } from "../../services/app/provider.tsx"
import { Badge } from "../app/badge.tsx"

const sourceMachine = createSourceMachine<Item>()

/** Render one Source using data selected from the root machine. */
export function SourceView() {
  /** Reference the root machine actor. */
  const appActor = useAppMachine()
  /** Read the active route. */
  const route = useSelector(appActor, (snapshot) => snapshot.context.route)
  /** Read the loaded Workspace. */
  const workspace = useSelector(appActor, (snapshot) => snapshot.context.executableWorkspace)
  /** Read Source items from machine context. */
  const sourceItems = useSelector(appActor, (snapshot) => snapshot.context.sourceItems)
  /** Read Source run results from machine context. */
  const sourceRuns = useSelector(appActor, (snapshot) => snapshot.context.sourceRuns)
  if (route.name !== "source" || !workspace) return null
  /** Resolve the Source named by the route. */
  const source = workspace.sources.find((candidate) => candidate.id === route.sourceId)
  if (!source) return null
  /** Resolve the Source data. */
  const items = sourceItems[source.id] ?? []
  const runResult = sourceRuns[source.id]
  /** Supply Source data to the selection machine. */
  const input: SourceMachineInput<Item> = {
    appActor,
    sourceId: source.id,
    items,
    getItemId: (item) => item.id,
  }
  /** Read the Workspace identifier. */
  const workspaceId = useSelector(appActor, (snapshot) => snapshot.context.executableWorkspace?.id ?? "No Workspace")
  /** Create the Source selection actor. */
  const actor = useActorRef(sourceMachine, { input })
  /** Read the Source selection snapshot. */
  const snapshot = useSelector(actor, (value) => value)
  /** Read the active theme. */
  const theme = useTheme()
  /** Read the normal item style. */
  const itemStyle = theme.component("source.item")
  /** Read the selected item style. */
  const selectedStyle = theme.component("source.item.selected")
  /** Read the Source summary style. */
  const summaryStyle = theme.component("source.summary")
  /** Read the Source configuration object. */
  const config = isRecord(source.source.config) ? source.source.config : {}
  /** Group Action results by Action step. */
  const actionResults = source.actions.map((_, actionIndex) =>
    runResult?.actions.filter((result) => result.actionIndex === actionIndex) ?? []
  )

  return (
    <KeymapScope actor={actor} bindings={sourceKeymap}>
      <box flexDirection="column" flexGrow={1}>
        <Breadcrumbs.Row>
          <Breadcrumbs.Item onClick={() => appActor.send({ type: "ROUTE_WORKSPACE" })}>
            <Badge.Type type="Workspace" label={workspaceId} />
          </Breadcrumbs.Item>
          <Breadcrumbs.Separator />
          <Breadcrumbs.Item>
            <Badge.Type type="Source" label={source.id} />
          </Breadcrumbs.Item>

        </Breadcrumbs.Row>


        <SourceSummaryCard
          sourceId={source.id}
          items={[...items]}
          actions={source.actions.map((action, index) => ({
            actionId: action.id ?? action.packageName,
            step: index + 1,
            items: (actionResults[index] ?? [])
              .map((result) => items.find((item) => item.id === result.itemId))
              .filter((item): item is Item => item !== undefined),
          }))}
        />
        <SourceDetailsCard
          source={source}
          config={config}
          borderColor={summaryStyle.border}
          foreground={summaryStyle.fg}
        />
        <ActionStepsCard
          appActor={appActor}
          source={source}
          items={items}
          results={actionResults}
          borderColor={summaryStyle.border}
          foreground={summaryStyle.fg}
        />
        <text marginTop={1}>Use Up and Down to select an item.</text>
        <text>Press Return to open the item.</text>
        <text>Press Escape to return to the workspace.</text>
        <box flexDirection="column" marginTop={1}>
          {snapshot.context.items.map((item, index) => (
            <box
              key={item.id}
              flexDirection="row"
              backgroundColor={(index === snapshot.context.itemIndex ? selectedStyle : itemStyle).bg}
            >
              <text fg={(index === snapshot.context.itemIndex ? selectedStyle : itemStyle).fg}>
                {index === snapshot.context.itemIndex ? "> " : "  "}{item.title} · {item.status}
              </text>
            </box>
          ))}
        </box>
      </box>
    </KeymapScope>
  )
}

function ActionStepsCard(props: {
  appActor: AnyActorRef
  source: LoadedWorkspaceSource
  items: readonly Item[]
  results: readonly ActionRunResult[][]
  borderColor?: string
  foreground?: string
}) {
  return (
    <box
      border={true}
      borderStyle="single"
      borderColor={props.borderColor}
      padding={1}
      marginBottom={1}
      flexDirection="column"
    >
      <text fg={props.foreground}>ACTIONS</text>
      {props.source.actions.map((action, actionIndex) => {
        const results = props.results[actionIndex] ?? []
        return (
          <box key={`${action.packageName}:${actionIndex}`} flexDirection="column" marginTop={1}>
            <text>{action.packageName}: {results.length} items</text>
            {results.length === 0 ? (
              <text>  No item results.</text>
            ) : results.map((result) => {
              const item = props.items.find((candidate) => candidate.id === result.itemId)
              return (
                <text
                  key={`${result.itemId}:${result.actionIndex}`}
                  onMouseDown={() => props.appActor.send({
                    type: "ROUTE_ACTION_ITEM",
                    sourceId: props.source.id,
                    itemId: result.itemId,
                    actionIndex: result.actionIndex,
                  })}
                >
                  [{item?.reference_id ?? result.itemId}] {item?.title ?? "Unknown item"} - {actionOutput(result)}
                </text>
              )
            })}
          </box>
        )
      })}
    </box>
  )
}

function actionOutput(result: ActionRunResult): string {
  const output = result.result?.stdout?.trim()
  if (output) return output.replace(/\s+/g, " ")
  if (result.error) return `error: ${result.error}`
  if (result.skipped) return "not executed"
  return "no stdout"
}

function SourceDetailsCard(props: {
  source: LoadedWorkspaceSource
  config: Record<string, unknown>
  borderColor?: string
  foreground?: string
}) {
  return (
    <box
      border={true}
      borderStyle="single"
      borderColor={props.borderColor}
      padding={1}
      marginBottom={1}
      flexDirection="column"
    >
      <text fg={props.foreground}>SOURCE DETAILS</text>
      <text>Plugin: {props.source.packageName}</text>
      {Object.entries(props.config).map(([key, value]) => (
        <text key={key}>{key}: {formatDetail(value)}</text>
      ))}
    </box>
  )
}

function formatDetail(value: unknown): string {
  if (typeof value === "string") return value
  if (value === undefined) return ""
  return JSON.stringify(value)
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value)
}

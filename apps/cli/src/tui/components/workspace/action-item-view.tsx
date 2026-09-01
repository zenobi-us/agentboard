import { useActorRef, useSelector } from "@xstate/react"

import type { SourceRunResult } from "../../../services/runtime.ts"
import { KeymapScope, actionItemKeymap } from "../../services/keymaps.tsx"
import { itemViewMachine } from "../../services/item/item.view.machine.ts"
import { useTheme } from "../../services/theme/theme.tsx"
import { useAppMachine } from "../../services/app/provider.tsx"
import { Breadcrumbs } from "../app/breadcrumbs.tsx"
import { Badge } from "../app/badge.tsx"
import { DefinitionGrid, DefinitionGridItem } from "../app/definition-grid.tsx"

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
  /** Resolve the selected Action. */
  const action = source.actions[route.actionIndex]
  if (!action) return null
  /** Create the Item navigation actor. */
  const actor = useActorRef(itemViewMachine, {
    input: {
      appActor,
      sourceId: source.id,
      itemId: item.id,
      item,
      openCommand: action.open,
      templateContext: {
        workspace: { id: workspace.id, path: workspace.path },
        source: {
          id: source.id,
          source: {
            uses: source.packageName,
            ...(source.source.config !== null && typeof source.source.config === "object"
              ? source.source.config
              : { value: source.source.config }),
          },
          actions: source.actions.map((configured) => ({
            id: configured.id,
            uses: configured.packageName,
            with: configured.config,
          })),
        },
        item,
        action: { index: route.actionIndex, uses: action.packageName },
        actions: {},
      },
    },
  })
  const snapshot = useSelector(actor, (value) => value)
  const theme = useTheme()
  const panelStyle = theme.component("source.summary")
  const result = runResult?.actions.find(
    (candidate) => candidate.itemId === item.id && candidate.actionIndex === route.actionIndex,
  )

  return (
    <KeymapScope actor={actor} bindings={actionItemKeymap}>
      <box flexDirection="column" flexGrow={1}>
        <Breadcrumbs.Row>
          <Breadcrumbs.Item onClick={() => appActor.send({ type: "ROUTE_WORKSPACE" })}>
            <Badge.Type type="Workspace" label={workspace.id} />
          </Breadcrumbs.Item>
          <Breadcrumbs.Separator />
          <Breadcrumbs.Item onClick={() => appActor.send({ type: "ROUTE_SOURCE", sourceId: source.id })}>
            <Badge.Type type="Source" label={source.id} />
          </Breadcrumbs.Item>
          <Breadcrumbs.Separator />
          <Breadcrumbs.Item onClick={() => appActor.send({ type: "ROUTE_ITEM", sourceId: source.id, itemId: item.id })}>
            <Badge.Type type="Item" label={item.reference_id} />
          </Breadcrumbs.Item>
          <Breadcrumbs.Separator />
          <Breadcrumbs.Item>
            <Badge.Type type="Action" label={`STEP ${route.actionIndex + 1}`} />
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
              <DefinitionGridItem label="Action">{action.id ?? action.packageName}</DefinitionGridItem>
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
          <text fg={panelStyle.fg}>EXECUTION</text>
          <box marginTop={1}>
            <DefinitionGrid>
              <DefinitionGridItem label="Step">{route.actionIndex + 1}</DefinitionGridItem>
              <DefinitionGridItem label="Uses">{action.packageName}</DefinitionGridItem>
              <DefinitionGridItem label="Outcome">{actionOutcome(result)}</DefinitionGridItem>
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

        {snapshot.context.openError ? <text>{snapshot.context.openError}</text> : null}
        <text>Press O to open the item. Press Escape to return to the source.</text>
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

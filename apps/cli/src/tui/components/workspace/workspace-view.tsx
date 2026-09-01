import { useActorRef, useSelector } from "@xstate/react"
import type { Item } from "@clankpipe/core/config"
import { KeymapScope, workspaceKeymap } from "../../services/keymaps.tsx"
import { workspaceMachine } from "../../services/workspace/workspace.machine.ts"
import { AppMachineContext } from "../../services/app/provider.tsx"
import type { LoadedWorkspace, LoadedWorkspaceSource } from "../../../services/config/workspace.ts"
import { pipelineStateLabel, type PipelineExecution } from "../../../services/store.ts"
import { Breadcrumbs } from "../app/breadcrumbs.tsx"
import { Badge } from "../app/badge.tsx"
import { Loader } from "../app/loader.tsx"
import { useTheme } from "../../services/theme/theme.tsx"

/** Render the Workspace as a source and pipeline tree. */
export function WorkspaceView(props: {
  workspace: LoadedWorkspace
  sourceItems: Record<string, readonly Item[]>
  pipelineExecutions: Record<string, readonly PipelineExecution[]>
}) {
  const appActor = AppMachineContext.useActorRef()
  const actor = useActorRef(workspaceMachine, {
    input: { appActor, sources: props.workspace.sources.map((source) => source.id) },
  })
  const snapshot = useSelector(actor, (value) => value)
  const theme = useTheme()
  const sourceStyle = theme.component("source.tree")
  const sourceIdStyle = theme.component("source.summary.id")
  const selectedStyle = theme.component("source.item.selected")
  const actionStyle = theme.component("source.summary.action")

  return (
    <KeymapScope actor={actor} bindings={workspaceKeymap}>
      <Breadcrumbs.Row>
        <Breadcrumbs.Item>
          <Badge.Type type="Workspace" label={props.workspace.id} />
        </Breadcrumbs.Item>
      </Breadcrumbs.Row>

      <box flexDirection="column" flexGrow={1}>
        {snapshot.context.sources.map((sourceId, index) => {
          const source = props.workspace.sources.find((item) => item.id === sourceId)
          if (!source) return null
          const groups = pipelineGroups(props.sourceItems[sourceId] ?? [], props.pipelineExecutions[sourceId] ?? [])

          return (
            <box key={sourceId} flexDirection="column" marginBottom={1}>
              <box
                flexDirection="row"
                onMouseDown={() => appActor.send({ type: "ROUTE_SOURCE", sourceId })}
              >
                <text fg={index === snapshot.context.sourceIndex ? selectedStyle.fg : sourceIdStyle.fg}>
                  {index === snapshot.context.sourceIndex ? "> " : "- "}{sourceId}
                </text>
              </box>
              {groups.map((group) => (
                <PipelineGroupView
                  key={`${sourceId}:${group.state}`}
                  source={source}
                  group={group}
                  actionStyle={actionStyle}
                  foreground={sourceStyle.fg}
                  onActionItem={(itemId, actionIndex) => appActor.send({
                    type: "ROUTE_ACTION_ITEM",
                    sourceId,
                    itemId,
                    actionIndex,
                  })}
                />
              ))}
            </box>
          )
        })}
      </box>
    </KeymapScope>
  )
}

type PipelineItem = {
  item: Item
  execution?: PipelineExecution
}

type PipelineGroup = {
  state: string
  items: PipelineItem[]
}

function pipelineGroups(
  sourceItems: readonly Item[],
  executions: readonly PipelineExecution[],
): PipelineGroup[] {
  const byItem = new Map<string, PipelineItem>()
  for (const item of sourceItems) byItem.set(item.id, { item })
  for (const execution of executions) byItem.set(execution.item_id, { item: execution.item, execution })

  const groups = new Map<string, PipelineGroup>()
  for (const value of byItem.values()) {
    const state = value.execution?.state ?? "ready"
    const group = groups.get(state) ?? { state, items: [] }
    group.items.push(value)
    groups.set(state, group)
  }

  return [...groups.values()].sort((left, right) => pipelineStateOrder(left.state) - pipelineStateOrder(right.state))
}

function pipelineStateOrder(state: string): number {
  return ["ready", "claimed", "running", "failed", "cancelled", "stale", "succeeded"].indexOf(state)
}

function PipelineGroupView(props: {
  source: LoadedWorkspaceSource
  group: PipelineGroup
  actionStyle: { fg?: string }
  foreground?: string
  onActionItem: (itemId: string, actionIndex: number) => void
}) {
  const actionGroups = new Map<number, { actionIndex: number; actionName: string; items: PipelineItem[] }>()
  for (const value of props.group.items) {
    if (props.source.actions.length === 0 || !value.execution) continue
    const actionIndex = value.execution.action_index ?? 0
    const action = props.source.actions[actionIndex]
    const actionName = action?.id ?? action?.packageName ?? "pipeline"
    const group = actionGroups.get(actionIndex) ?? { actionIndex, actionName, items: [] }
    group.items.push(value)
    actionGroups.set(actionIndex, group)
  }

  return (
    <box flexDirection="column" marginLeft={2}>
      <text fg={props.foreground}>- {pipelineStateLabel(props.group.state)} {props.group.items.length} items</text>
      {[...actionGroups.values()].map((actionGroup) => (
        <box key={actionGroup.actionIndex} flexDirection="column" marginLeft={3}>
          <text fg={props.actionStyle.fg}>- {actionGroup.actionName}</text>
          {actionGroup.items.map(({ item, execution }) => (
            <box
              key={`${actionGroup.actionIndex}:${item.id}`}
              flexDirection="row"
              marginLeft={3}
              onMouseDown={() => props.onActionItem(item.id, actionGroup.actionIndex)}
            >
              {props.group.state === "running" ? <Loader size="sm" /> : <text>·</text>}
              <text> {item.reference_id} - {item.title}</text>
              {execution?.message ? <text> ({execution.message})</text> : null}
            </box>
          ))}
        </box>
      ))}
    </box>
  )
}

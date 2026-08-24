import { useActorRef, useSelector } from "@xstate/react"
import { SourceSummaryCard } from "./source-summary-card.tsx"
import { KeymapScope, workspaceKeymap } from "../../services/keymaps.tsx"
import { workspaceMachine } from "../../services/workspace/workspace.machine.ts"
import { AppMachineContext } from "../../services/app/provider.tsx"
import type { LoadedWorkspace } from "../../../services/config/workspace.ts"

/**
 * Workspace view lists all sources in the current workspace.
 *
 * Selecting a source, navigates to the source view.
 */
export function WorkspaceView(props: { workspace: LoadedWorkspace }) {
  const appActor = AppMachineContext.useActorRef()
  const actor = useActorRef(workspaceMachine, {
    input: { appActor, sources: props.workspace.sources.map((source) => source.id) },
  })
  const snapshot = useSelector(actor, (value) => value)

  return (
    <KeymapScope actor={actor} bindings={workspaceKeymap}>
      <box flexDirection="column" flexGrow={1}>
        {snapshot.context.sources.map((sourceId) => {
          const source = props.workspace.sources.find((item) => item.id === sourceId)

          return (
            <SourceSummaryCard
              key={sourceId}
              sourceId={sourceId}
              items={[]}
              actions={(source?.actions ?? []).map((action, index) => ({
                actionId: action.id ?? action.packageName,
                step: index + 1,
                items: [],
              }))}
              onClick={() => appActor.send({ type: "ROUTE_SOURCE", sourceId })}
            />
          )
        })}
      </box>
    </KeymapScope>
  )
}

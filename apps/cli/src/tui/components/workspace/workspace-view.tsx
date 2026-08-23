import { useActorRef, useSelector } from "@xstate/react"
import { SourceSummaryCard } from "./source-summary-card.tsx"
import { KeymapScope, workspaceKeymap } from "../../services/keymaps.tsx"
import { workspaceMachine } from "../../services/workspace/workspace.machine.ts"
import { AppMachineContext } from "../../services/app/provider.tsx"

/**
 * Workspace view lists all sources in the current workspace.
 *
 * Selecting a source, navigates to the source view.
 */
export function WorkspaceView() {
  const appActor = AppMachineContext.useActorRef()
  const actor = useActorRef(workspaceMachine, { input: { appActor } })
  const snapshot = useSelector(actor, (value) => value)
  const sourceId = snapshot.context.sources[snapshot.context.sourceIndex] ?? "unknown"

  return (
    <KeymapScope actor={actor} bindings={workspaceKeymap}>
      <box flexDirection="column" flexGrow={1}>
        <SourceSummaryCard
          sourceId={sourceId}
          items={[]}
          actions={[]}
          key={sourceId}
        />
      </box>
    </KeymapScope>
  )
}

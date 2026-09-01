import type { Item } from "@clankpipe/core/config"
import type { LoadedWorkspace } from "../../../services/config/workspace.ts"
import type { PipelineExecution } from "../../../services/store.ts"
import { useTheme } from "../../services/theme/theme.tsx"

export function ItemsView(props: {
  workspace: LoadedWorkspace
  sourceItems: Record<string, readonly Item[]>
  pipelineExecutions: Record<string, readonly PipelineExecution[]>
}) {
  const tableStyle = useTheme().component("source.table")
  const rows = props.workspace.sources.flatMap((source) =>
    (props.sourceItems[source.id] ?? []).map((item) => ({ sourceId: source.id, item }))
  )

  return (
    <box flexDirection="column" flexGrow={1}>
      <text fg={tableStyle.fg}>ITEMS</text>
      <text marginTop={1}>Source results and active pipeline Items.</text>
      {rows.length === 0 ? (
        <text marginTop={1}>No collected items. Press L to list the current sources.</text>
      ) : (
        <box flexDirection="column" marginTop={1}>
          <text fg={tableStyle.fg}>SOURCE\tREFERENCE\tSTATUS\tPIPELINE\tTITLE</text>
          {rows.map(({ sourceId, item }) => (
            <text key={`${sourceId}:${item.id}`}>
              {sourceId}\t{item.reference_id}\t{item.status}\t{pipelineState(props.pipelineExecutions[sourceId], item.id)}\t{item.title}
            </text>
          ))}
        </box>
      )}
    </box>
  )
}

function pipelineState(executions: readonly PipelineExecution[] | undefined, itemId: string): string {
  return executions?.find((execution) => execution.item_id === itemId)?.state ?? "ready"
}

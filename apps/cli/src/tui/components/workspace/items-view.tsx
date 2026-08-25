import type { Item } from "@agentboard/core/config"
import type { LoadedWorkspace } from "../../../services/config/workspace.ts"
import { useTheme } from "../../services/theme/theme.tsx"

export function ItemsView(props: {
  workspace: LoadedWorkspace
  sourceItems: Record<string, readonly Item[]>
}) {
  const tableStyle = useTheme().component("source.table")
  const rows = props.workspace.sources.flatMap((source) =>
    (props.sourceItems[source.id] ?? []).map((item) => ({ sourceId: source.id, item }))
  )

  return (
    <box flexDirection="column" flexGrow={1}>
      <text fg={tableStyle.fg}>ITEMS</text>
      <text marginTop={1}>Source results only. Actions are not run in this view.</text>
      {rows.length === 0 ? (
        <text marginTop={1}>No collected items. Press L to list the current sources.</text>
      ) : (
        <box flexDirection="column" marginTop={1}>
          <text fg={tableStyle.fg}>SOURCE	REFERENCE	STATUS	TITLE</text>
          {rows.map(({ sourceId, item }) => (
            <text key={`${sourceId}:${item.id}`}>
              {sourceId}	{item.reference_id}	{item.status}	{item.title}
            </text>
          ))}
        </box>
      )}
    </box>
  )
}
import { useTheme } from "../../services/theme/theme.tsx"

type SourceSummaryAction<T> = {
  actionId: string;
  step: number;
  items: T[];
}

type SourceSummary<T, A extends SourceSummaryAction<T>> = {
  sourceId: string;
  items: T[];
  actions: A[];
  onClick?: () => void;
}

/**
 * Source Summary Card component for displaying a summary of a source, including its ID, number of items, and actions.
 *
 * It shows the source, the items in the pipeline and for each action it shows:
 * - the action,
 * - the number of items at that step
 */
export function SourceSummaryCard<T, A extends SourceSummaryAction<T>>(props: SourceSummary<T, A>) {
  const theme = useTheme()
  const summaryStyle = theme.component("source.summary")
  const idStyle = theme.component("source.summary.id")

  return (
    <box
      border={true}
      borderStyle="single"
      borderColor={summaryStyle.border}
      padding={1}
      marginBottom={1}
      flexDirection="row"
      justifyContent="flex-start"
      onMouseDown={props.onClick}
    >
      <box flexDirection="column" marginRight={2} >
        <text fg={idStyle.fg}>{props.sourceId}</text>
        <text>{props.items.length} items</text>
      </box>
      <box flexDirection="row">
        {props.actions.length > 0 && props.actions.map((action) => (
          <SourceSummaryActionStep
            actionId={action.actionId}
            step={action.step}
            items={action.items}
            key={action.actionId}
          />
        ))}
      </box>
    </box>
  )
}




function SourceSummaryActionStep<T>(props: { actionId: string; step: number, items: T[] }) {
  const theme = useTheme()
  const actionStyle = theme.component("source.summary.action")

  return (
    <box flexDirection="column" marginRight={2}>
      <box flexDirection="row" alignItems="center">
        <text paddingRight={1}>{props.step}</text>
        <text fg={actionStyle.fg}>{props.actionId}</text>
      </box>

      {props.items.length > 0 && (
        <text>{props.items.length} items</text>
      )}
    </box>
  )
}

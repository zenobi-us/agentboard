type SourceSummaryAction<T> = {
  actionId: string;
  step: number;
  items: T[];
}

type SourceSummary<T, A extends SourceSummaryAction<T>> = {
  sourceId: string;
  items: T[];
  actions: A[];
}

/**
 * Source Summary Card component for displaying a summary of a source, including its ID, number of items, and actions.
 *
 * It shows the source, the items in the pipeline and for each action it shows:
 * - the action,
 * - the number of items at that step
 */
export function SourceSummaryCard<T, A extends SourceSummaryAction<T>>(props: SourceSummary<T, A>) {
  return (
    <box border={true} borderStyle="single" padding={1} marginBottom={1} flexDirection="row" justifyContent="space-between">
      <box flexDirection="column" justifyContent="space-between">
        <text fg="#f2c94c">{props.sourceId}</text>
        <text>{props.items.length} items</text>
      </box>
      <box flexDirection="row" marginTop={1}>
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
  return (
    <box flexDirection="column" marginRight={2}>
      <text fg="#8d99ae">{props.actionId}</text>
      <text>{props.step} steps</text>
      {props.items.length > 0 && (
        <text>{props.items.length} items</text>
      )}
    </box>
  )
}

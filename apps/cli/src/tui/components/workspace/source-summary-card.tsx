import { useTheme } from "../../services/theme/theme.tsx"

type SourceSummaryAction<T> = {
  actionId: string;
  step: number;
  items: T[];
}

type SourceSummary<T, A extends SourceSummaryAction<T>> = {
  sourceId: string;
  items: T[];
  pipeline?: readonly { state: string }[];
  actions: A[];
  selected?: boolean;
  onClick?: () => void;
}

export function SourceSummaryCard<T, A extends SourceSummaryAction<T>>(props: SourceSummary<T, A>) {
  const theme = useTheme()
  const summaryStyle = theme.component("source.summary")
  const idStyle = theme.component("source.summary.id")
  const selectedStyle = theme.component("source.item.selected")

  return (
    <box
      border={true}
      borderStyle="single"
      borderColor={props.selected ? selectedStyle.border : summaryStyle.border}
      backgroundColor={props.selected ? selectedStyle.bg : summaryStyle.bg}
      padding={1}
      marginBottom={1}
      flexDirection="row"
      justifyContent="flex-start"
      onMouseDown={props.onClick}
    >
      <box flexDirection="column" marginRight={2}>
        <text fg={props.selected ? selectedStyle.fg : idStyle.fg}>{props.sourceId}</text>
        <text>{props.items.length} items</text>
        {props.pipeline && props.pipeline.length > 0 && <text>{pipelineSummary(props.pipeline)}</text>}
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

function pipelineSummary(executions: readonly { state: string }[]): string {
  const counts = new Map<string, number>()
  for (const execution of executions) counts.set(execution.state, (counts.get(execution.state) ?? 0) + 1)
  return [...counts.entries()].map(([state, count]) => `${state}: ${count}`).join(", ")
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

      {props.items.length > 0 && <text>{props.items.length} items</text>}
    </box>
  )
}

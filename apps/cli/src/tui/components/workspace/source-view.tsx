import { useActorRef, useSelector } from "@xstate/react"
import type { Item } from "@agentboard/core/config"
import type { AnyActorRef } from "xstate"
import { KeymapScope, sourceKeymap } from "../../services/keymaps.tsx"
import { createSourceMachine, type SourceMachineInput } from "../../services/source/source.machine.ts"
import { useTheme } from "../../services/theme/theme.tsx"
import { SourceSummaryCard } from "./source-summary-card.tsx"
import type { LoadedWorkspaceSource } from "../../../services/config/workspace.ts"

const sourceMachine = createSourceMachine<Item>()

type SourceViewProps = {
  appActor: AnyActorRef
  source: LoadedWorkspaceSource
  items: readonly Item[]
}

export function SourceView(props: SourceViewProps) {
  const input: SourceMachineInput<Item> = {
    appActor: props.appActor,
    sourceId: props.source.id,
    items: props.items,
    getItemId: (item) => item.id,
  }
  const actor = useActorRef(sourceMachine, { input })
  const snapshot = useSelector(actor, (value) => value)
  const theme = useTheme()
  const headingStyle = theme.component("source.heading")
  const itemStyle = theme.component("source.item")
  const selectedStyle = theme.component("source.item.selected")
  const summaryStyle = theme.component("source.summary")
  const config = isRecord(props.source.source.config) ? props.source.source.config : {}

  return (
    <KeymapScope actor={actor} bindings={sourceKeymap}>
      <box flexDirection="column" flexGrow={1}>
        <text fg={headingStyle.fg}>SOURCE / {snapshot.context.sourceId}</text>
        <SourceSummaryCard
          sourceId={props.source.id}
          items={[...props.items]}
          actions={props.source.actions.map((action, index) => ({
            actionId: action.id ?? action.packageName,
            step: index + 1,
            items: [],
          }))}
        />
        <SourceDetailsCard
          source={props.source}
          config={config}
          borderColor={summaryStyle.border}
          foreground={summaryStyle.fg}
        />
        <text marginTop={1}>Use Up and Down to select an item.</text>
        <text>Press Return to open the item.</text>
        <text>Press Escape to return to the workspace.</text>
        <box flexDirection="column" marginTop={1}>
          {snapshot.context.items.map((item, index) => (
            <text
              key={item.id}
              fg={(index === snapshot.context.itemIndex ? selectedStyle : itemStyle).fg}
            >
              {index === snapshot.context.itemIndex ? "> " : "  "}{item.title} · {item.status}
            </text>
          ))}
        </box>
      </box>
    </KeymapScope>
  )
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

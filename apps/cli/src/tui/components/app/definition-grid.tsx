import { useTheme } from "../../services/theme/theme.tsx"

export function DefinitionGrid(props: { children: React.ReactNode }) {
  return <box flexDirection="column">{props.children}</box>
}

export function DefinitionGridItem(props: { label: string; children: React.ReactNode }) {
  const theme = useTheme()
  const labelStyle = theme.component("definition.label")

  return (
    <box flexDirection="row">
      <box width={14}>
        <text fg={labelStyle.fg}>{props.label}</text>
      </box>
      <box flexGrow={1}>
        <text>{props.children}</text>
      </box>
    </box>
  )
}

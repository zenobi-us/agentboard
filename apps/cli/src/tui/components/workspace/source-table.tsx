import { useTheme } from "../../services/theme/theme.tsx"

export function SourceTable(props: {
  items: readonly string[]
  selectedIndex: number
}) {
  const theme = useTheme()
  const tableStyle = theme.component("source.table")
  const itemStyle = theme.component("source.item")
  const selectedStyle = theme.component("source.item.selected")

  return (
    <box flexDirection="column" flexGrow={1}>
      <text fg={tableStyle.fg}>ITEMS</text>
      {props.items.map((item, index) => (
        <text key={item} fg={(index === props.selectedIndex ? selectedStyle : itemStyle).fg}>
          {index === props.selectedIndex ? "> " : "  "}{item}
        </text>
      ))}
    </box>
  )
}

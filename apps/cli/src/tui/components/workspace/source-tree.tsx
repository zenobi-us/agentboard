import { useTheme } from "../../services/theme/theme.tsx"

export function SourceTree(props: {
  sources: readonly string[]
  selectedIndex: number
}) {
  const theme = useTheme()
  const treeStyle = theme.component("source.tree")
  const itemStyle = theme.component("source.item")
  const selectedStyle = theme.component("source.item.selected")

  return (
    <box width={20} flexDirection="column" marginRight={2}>
      <text fg={treeStyle.fg}>SOURCES</text>
      {props.sources.map((source, index) => (
        <text key={source} fg={(index === props.selectedIndex ? selectedStyle : itemStyle).fg}>
          {index === props.selectedIndex ? "> " : "  "}{source}
        </text>
      ))}
    </box>
  )
}

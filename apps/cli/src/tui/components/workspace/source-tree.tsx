export function SourceTree(props: {
  sources: readonly string[]
  selectedIndex: number
}) {
  return (
    <box width={20} flexDirection="column" marginRight={2}>
      <text fg="#8d99ae">SOURCES</text>
      {props.sources.map((source, index) => (
        <text key={source} fg={index === props.selectedIndex ? "#f2c94c" : "#d8dee9"}>
          {index === props.selectedIndex ? "> " : "  "}{source}
        </text>
      ))}
    </box>
  )
}

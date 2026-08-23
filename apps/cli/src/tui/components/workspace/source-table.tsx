export function SourceTable(props: {
  items: readonly string[]
  selectedIndex: number
}) {
  return (
    <box flexDirection="column" flexGrow={1}>
      <text fg="#8d99ae">ITEMS</text>
      {props.items.map((item, index) => (
        <text key={item} fg={index === props.selectedIndex ? "#f2c94c" : "#d8dee9"}>
          {index === props.selectedIndex ? "> " : "  "}{item}
        </text>
      ))}
    </box>
  )
}

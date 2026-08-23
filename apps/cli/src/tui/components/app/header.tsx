export function AppHeader() {
  return (
    <box flexDirection="row" border={false} marginBottom={1}>
      <box flexDirection="row" flexGrow={1}>
        <text >AgentBoard</text>
      </box>
      <box flexDirection="row" border={false}>
        <text>v{process.env.AGENTBOARD_VERSION}</text>
      </box>
    </box>
  )
}

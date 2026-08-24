import { useSelector } from "@xstate/react";
import { useAppMachine } from "../../services/app/provider"

export function AppHeader() {
  const appMachine = useAppMachine()
  const version = useSelector(appMachine, (snapshot) => snapshot.context.version)

  return (
    <box flexDirection="row" border={false} marginBottom={1}>
      <box flexDirection="row" flexGrow={1}>
        <text>AgentBoard</text>
      </box>
      <box flexDirection="row" border={false} marginLeft={1}>
        <text>v{version}</text>
      </box>
    </box>
  )
}

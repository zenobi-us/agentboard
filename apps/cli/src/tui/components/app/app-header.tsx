import { useSelector } from "@xstate/react";
import { useAppMachine } from "../../services/app/provider"
import { useTheme } from "../../services/theme/theme.tsx"

export function AppHeader(props: {
  children?: React.ReactNode
}) {
  const appMachine = useAppMachine()
  const theme = useTheme()
  const headerStyle = theme.component("app.header")
  const version = useSelector(appMachine, (snapshot) => snapshot.context.version)
  return (
    <box
      flexDirection="column"
      border={["bottom"]}
      borderColor={headerStyle.border}
      marginBottom={1}
      flexGrow={1}
    >
      <box flexDirection="row" border={false} marginBottom={1}>
        <box flexDirection="row" flexGrow={1}>
          <text>AgentBoard</text>
        </box>
        <box flexDirection="row" border={false} marginLeft={1}>
          <text>v{version}</text>
        </box>
      </box>
      {props.children}
    </box>
  )
}

export function MainTabs() {
  const appMachine = useAppMachine()
  const route = useSelector(appMachine, (snapshot) => snapshot.context.route)
  const theme = useTheme()
  const selected = theme.component("source.summary.id")
  const idle = theme.component("source.item")

  return (
    <box flexDirection="row" border={false}>
      <text
        fg={(route.name === "items" ? idle : selected).fg}
        onMouseDown={() => appMachine.send({ type: "ROUTE_WORKSPACE" })}
      >
        {route.name === "items" ? "  Workspace  " : "[ Workspace ]"}
      </text>
      <text
        fg={(route.name === "items" ? selected : idle).fg}
        onMouseDown={() => appMachine.send({ type: "ROUTE_ITEMS" })}
      >
        {route.name === "items" ? "[ Items ]" : "  Items  "}
      </text>
    </box>
  )
}

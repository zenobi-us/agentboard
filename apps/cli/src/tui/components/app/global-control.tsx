import { useSelector } from "@xstate/react"
import { ActionButton } from "./action-button.tsx"
import { useAppMachine } from "../../services/app/provider.tsx"

export function GlobalControl() {
  const appMachine = useAppMachine()
  const runRequest = useSelector(appMachine, (snapshot) => snapshot.context.runRequest)
  const runActive = runRequest.mode === "run"
  const watchActive = runRequest.mode === "watch"
  const listActive = runRequest.mode === "list"

  return (
    <box flexDirection="row" border={false}>
      <ActionButton
        shortcut="r"
        label={runActive ? "Stop" : "Run"}
        themeToken="button.run"
        loading={runActive}
        disabled={watchActive || listActive || (runActive && runRequest.stopping)}
        onPress={() => appMachine.send({ type: "COMMAND", code: runActive ? "app.stop" : "app.run" })}
      />
      <ActionButton
        shortcut="w"
        label={watchActive ? "Stop" : "Watch"}
        themeToken="button.watch"
        loading={watchActive}
        disabled={runActive || listActive || (watchActive && runRequest.stopping)}
        onPress={() => appMachine.send({ type: "COMMAND", code: watchActive ? "app.stop" : "app.watch" })}
      />
      <ActionButton
        shortcut="l"
        label={listActive ? "Stop" : "List"}
        themeToken="button.list"
        loading={listActive}
        disabled={runActive || watchActive || (listActive && runRequest.stopping)}
        onPress={() => appMachine.send({ type: "COMMAND", code: listActive ? "app.stop" : "app.list" })}
      />
    </box>
  )
}

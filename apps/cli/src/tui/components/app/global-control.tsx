import { useSelector } from "@xstate/react"
import { ActionButton } from "./action-button.tsx"
import { useAppMachine } from "../../services/app/provider.tsx"

export function GlobalControl() {
  const appMachine = useAppMachine()
  const runRequest = useSelector(appMachine, (snapshot) => snapshot.context.runRequest)
  const runActive = runRequest.mode === "run"
  const watchActive = runRequest.mode === "watch"

  return (
    <box flexDirection="row" border={false}>
      <ActionButton
        shortcut="r"
        label={runActive ? "Stop" : "Run"}
        themeToken="button.run"
        loading={runActive}
        disabled={watchActive || (runActive && runRequest.stopping)}
        onPress={() => appMachine.send({ type: "COMMAND", code: runActive ? "app.stop" : "app.run" })}
      />
      <ActionButton
        shortcut="w"
        label={watchActive ? "Stop" : "Watch"}
        themeToken="button.watch"
        loading={watchActive}
        disabled={runActive || (watchActive && runRequest.stopping)}
        onPress={() => appMachine.send({ type: "COMMAND", code: watchActive ? "app.stop" : "app.watch" })}
      />
    </box>
  )
}

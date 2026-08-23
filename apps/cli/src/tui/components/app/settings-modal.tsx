import { useActorRef, useSelector } from "@xstate/react"
import { KeymapScope, modalKeymap } from "../../services/keymaps.tsx"
import { modalMachine } from "../../services/app/modal.machine.ts"
import { AppMachineContext } from "../../services/app/provider.tsx"

export function SettingsModal() {
  const appActor = AppMachineContext.useActorRef()
  const modalActor = useActorRef(modalMachine)
  const closed = useSelector(modalActor, (snapshot) => snapshot.matches("closed"))

  if (closed) {
    appActor.send({ type: "MODAL_CLOSED" })
    return null
  }

  return (
    <KeymapScope actor={modalActor} bindings={modalKeymap}>
      <box
        position="absolute"
        left={0}
        top={0}
        bottom={0}
        width={34}
        zIndex={10}
        border={true}
        borderStyle="single"
        backgroundColor="#171a21"
        padding={1}
        flexDirection="column"
      >
        <box flexDirection="row" alignItems="flex-start" flexGrow={1}>
          <text fg="#f2c94c">SETTINGS</text>
          <text marginTop={1}>This panel owns its keymap scope.</text>
        </box>
        <box flexDirection="row" marginTop={1}>
          <text marginTop={1} fg="#8d99ae">Esc  Close</text>
          <text fg="#8d99ae">Ctrl+S  Save</text>
        </box>
      </box>
    </KeymapScope>
  )
}

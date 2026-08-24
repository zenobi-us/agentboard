
import { createActorContext } from '@xstate/react'

import { tuiMachine } from './machine'

export const AppMachineContext = createActorContext(tuiMachine, {})

export function AppMachineProvider(props: {
  workspacePath: string
  version: string
  children: React.ReactNode
}) {
  return (
    <AppMachineContext.Provider options={{ input: { workspacePath: props.workspacePath, version: props.version } }}>
      {props.children}
    </AppMachineContext.Provider>
  )
}

export function useAppMachine() {
  return AppMachineContext.useActorRef()
}

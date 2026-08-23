
import { createActorContext } from '@xstate/react'

import { tuiMachine } from './machine'

export const AppMachineContext = createActorContext(tuiMachine, {})

export function AppMachineProvider(props: {
  children: React.ReactNode
}) {
  return <AppMachineContext.Provider>{props.children}</AppMachineContext.Provider>
}

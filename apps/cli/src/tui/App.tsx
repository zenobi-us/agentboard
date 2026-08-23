import { useSelector } from "@xstate/react"
// @ts-expect-error The TUI currently has no local React type package.
import { useEffect } from "react"
import { useRenderer } from "@opentui/react"
import { AppHeader } from "./components/app/header.tsx"
import { AppLayout } from "./components/app/layout.tsx"
import { SettingsModal } from "./components/app/settings-modal.tsx"
import { ItemView } from "./components/workspace/item-view.tsx"
import { Loader } from "./components/app/loader.tsx";
import {
  SourceView,
  type DemoSourceItem,
} from "./components/workspace/source-view.tsx"
import { WorkspaceView } from "./components/workspace/workspace-view.tsx"
import { AppMachineContext, AppMachineProvider } from "./services/app/provider.tsx"
import { KeymapScope, KeymapProvider, appKeymap } from "./services/keymaps.tsx"

const demoSourceItems: readonly DemoSourceItem[] = [
  { id: "item-1", title: "Fix sync failure" },
  { id: "item-2", title: "Review source mapping" },
  { id: "item-3", title: "Update dashboard" },
]

function AppScreen() {
  const appActor = AppMachineContext.useActorRef()
  const renderer = useRenderer()
  const route = useSelector(appActor, (snapshot) => snapshot.context.route)
  const settingsOpen = useSelector(appActor, (snapshot) => snapshot.matches("settings"))
  const exiting = useSelector(appActor, (snapshot) => snapshot.matches("exiting"))
  const initialising = useSelector(appActor, (snapshot) => snapshot.matches("initialising"))

  useEffect(() => {
    if (exiting) renderer.destroy()
  }, [exiting, renderer])


  if (initialising) {
    return (
      <box flexDirection="column" alignItems="center" justifyContent="center" flexGrow={1} padding={1}>
        <Loader size="lg" />
      </box>
    )
  }

  return (
    <KeymapScope actor={appActor} bindings={appKeymap}>
      <AppLayout
        header={<AppHeader />}
        footer={<text>Ctrl+S Settings · Ctrl+R Refresh · Ctrl+Q Quit</text>}
      >
        <box position="relative" flexGrow={1} padding={1}>
          {route.name === "workspace" ? <WorkspaceView /> : null}
          {route.name === "source" ? (
            <SourceView
              key={route.sourceId}
              appActor={appActor}
              sourceId={route.sourceId}
              items={demoSourceItems}
              getItemId={(item) => item.id}
              getItemLabel={(item) => item.title}
            />
          ) : null}
          {route.name === "item" ? (
            <ItemView
              key={`${route.sourceId}:${route.itemId}`}
              appActor={appActor}
              sourceId={route.sourceId}
              itemId={route.itemId}
            />
          ) : null}
          {settingsOpen ? <SettingsModal /> : null}
        </box>
      </AppLayout>
    </KeymapScope>
  )
}

export function App() {
  return (
    <AppMachineProvider>
      <KeymapProvider>
        <AppScreen />
      </KeymapProvider>
    </AppMachineProvider>
  )
}

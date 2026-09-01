import { useSelector } from "@xstate/react"
// @ts-expect-error The TUI currently has no local React type package.
import { useEffect } from "react"

import { useRenderer } from "@opentui/react"
import { AppHeader, MainTabs } from "./components/app/app-header.tsx"
import { AppLayout } from "./components/app/layout.tsx"
import { SettingsModal } from "./components/app/settings-modal.tsx"
import { ItemView } from "./components/workspace/item-view.tsx"
import { ActionItemView } from "./components/workspace/action-item-view.tsx"
import { Loader } from "./components/app/loader.tsx";
import { SourceView } from "./components/workspace/source-view.tsx"
import { WorkspaceView } from "./components/workspace/workspace-view.tsx"
import { ItemsView } from "./components/workspace/items-view.tsx"

import { AppMachineContext, AppMachineProvider } from "./services/app/provider.tsx"
import { KeymapScope, KeymapProvider, appKeymap } from "./services/keymaps.tsx"
import { GlobalControl } from "./components/app/global-control.tsx"
import { loadTheme, ThemeProvider } from "./services/theme/theme.tsx"
import { ToastProvider, useToast } from "./services/toast/toast.tsx"

/** Render the TUI from the root machine snapshot. */
function AppScreen() {
  /** Reference the root actor that owns every TUI operation. */
  const appActor = AppMachineContext.useActorRef()
  /** Reference the OpenTUI renderer for shutdown. */
  const renderer = useRenderer()
  /** Read the active navigation route. */
  const route = useSelector(appActor, (snapshot) => snapshot.context.route)
  /** Read whether the settings overlay is active. */
  const settingsOpen = useSelector(appActor, (snapshot) => snapshot.matches("settings"))
  /** Read whether the root actor is exiting. */
  const exiting = useSelector(appActor, (snapshot) => snapshot.matches("exiting"))
  /** Read whether Workspace loading is active. */
  const initialising = useSelector(appActor, (snapshot) => snapshot.matches("initialising"))
  /** Read the Workspace loading error. */
  const loadingError = useSelector(appActor, (snapshot) => snapshot.context.error)
  /** Read the loaded Workspace used by route renderers. */
  const executableWorkspace = useSelector(appActor, (snapshot) => snapshot.context.executableWorkspace)
  /** Read Source items stored by the root machine. */
  const sourceItems = useSelector(appActor, (snapshot) => snapshot.context.sourceItems)
  const pipelineExecutions = useSelector(appActor, (snapshot) => snapshot.context.pipelineExecutions)
  /** Read the current Workspace operation error. */
  const runError = useSelector(appActor, (snapshot) => snapshot.context.runError)
  const { toast } = useToast()
  useEffect(() => {
    if (runError) toast({ message: runError, variant: "error", duration: 0 })
  }, [runError, toast])
  if (exiting) renderer.destroy()


  if (initialising) {
    return (
      <KeymapScope actor={appActor} bindings={appKeymap}>
        <box flexDirection="column" alignItems="center" justifyContent="center" flexGrow={1} padding={1}>
          <Loader size="lg" />
        </box>
      </KeymapScope>
    )
  }

  if (loadingError || !executableWorkspace) {
    return (
      <KeymapScope actor={appActor} bindings={appKeymap}>
        <box flexDirection="column" alignItems="center" justifyContent="center" flexGrow={1} padding={1}>
          <text>Workspace load failed</text>
          <text>{loadingError ?? "Workspace is not available"}</text>
          <text marginTop={1}>Press Ctrl+Q to quit</text>
        </box>
      </KeymapScope>
    )
  }

  return (
    <KeymapScope actor={appActor} bindings={appKeymap}>
      <AppLayout
        header={
          <AppHeader>
            <box flexDirection="row" flexGrow={1} justifyContent="space-between" width="100%">
              <MainTabs />
              <GlobalControl />
            </box>
          </AppHeader>
        }
        footer={< text > Ctrl + S Settings · Ctrl + R Refresh · Ctrl + Q Quit</text >}
      >
        <box flexDirection="column" border={false} marginBottom={1} flexGrow={1}>
          <box position="relative" flexGrow={1} padding={1}>
            {route.name === "workspace" ? <WorkspaceView workspace={executableWorkspace} sourceItems={sourceItems} pipelineExecutions={pipelineExecutions} /> : null}
            {route.name === "items" ? <ItemsView workspace={executableWorkspace} sourceItems={sourceItems} pipelineExecutions={pipelineExecutions} /> : null}
            {route.name === "source" ? <SourceView key={route.sourceId} /> : null}
            {route.name === "item" ? (<ItemView key={`${route.sourceId}:${route.itemId}`} />) : null}
            {route.name === "action-item" ? (<ActionItemView key={`${route.sourceId}:${route.itemId}:${route.actionIndex}`} />) : null}
            {settingsOpen ? <SettingsModal /> : null}
          </box>
        </box>
      </AppLayout >
    </KeymapScope >
  )
}

/** Compose providers around the machine-owned TUI. */
export function App(props: { workspacePath: string; version: string }) {
  /** Load presentation settings for this Workspace path. */
  const theme = loadTheme(props.workspacePath)

  return (
    <ThemeProvider theme={theme}>
      <ToastProvider>
        <AppMachineProvider workspacePath={props.workspacePath} version={props.version}>
          <KeymapProvider>
            <AppScreen />
          </KeymapProvider>
        </AppMachineProvider>
      </ToastProvider>
    </ThemeProvider>
  )
}

import { useSelector } from "@xstate/react"
// @ts-expect-error The TUI currently has no local React type package.
import { useEffect, useState } from "react"
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
import type { Item } from "@agentboard/core/config"
import { runWorkspace, watchWorkspace, type SourceRunResult, type WorkspaceRunResult } from "../services/runtime.ts"
import { AppMachineContext, AppMachineProvider } from "./services/app/provider.tsx"
import { KeymapScope, KeymapProvider, appKeymap } from "./services/keymaps.tsx"
import { GlobalControl } from "./components/app/global-control.tsx"
import { loadTheme, ThemeProvider, useTheme } from "./services/theme/theme.tsx"

function AppScreen() {
  const appActor = AppMachineContext.useActorRef()
  const renderer = useRenderer()
  const route = useSelector(appActor, (snapshot) => snapshot.context.route)
  const settingsOpen = useSelector(appActor, (snapshot) => snapshot.matches("settings"))
  const exiting = useSelector(appActor, (snapshot) => snapshot.matches("exiting"))
  const initialising = useSelector(appActor, (snapshot) => snapshot.matches("initialising"))
  const loadingError = useSelector(appActor, (snapshot) => snapshot.context.error)
  const executableWorkspace = useSelector(appActor, (snapshot) => snapshot.context.executableWorkspace)
  const runRequest = useSelector(appActor, (snapshot) => snapshot.context.runRequest)
  const theme = useTheme()
  const errorStyle = theme.component("error")
  const [sourceItems, setSourceItems] = useState<Record<string, readonly Item[]>>({})
  const [sourceRuns, setSourceRuns] = useState<Record<string, SourceRunResult>>({})
  const [runError, setRunError] = useState<string>()

  useEffect(() => {
    if (!executableWorkspace || runRequest.mode === "idle") return

    let active = true
    const controller = new AbortController()
    setRunError(undefined)
    const listSourceId = runRequest.mode === "list" && route.name === "source" ? route.sourceId : undefined
    const executionWorkspace = {
      ...executableWorkspace,
      cancellation: controller.signal,
      sources: executableWorkspace.sources.map((source) => ({ ...source, cancellation: controller.signal })),
    }
    const applyResult = (result: WorkspaceRunResult) => {
      if (!active) return
      setSourceItems((current: Record<string, readonly Item[]>) => ({
        ...current,
        ...Object.fromEntries(result.sources.map((source) => [source.id, source.items])),
      }))
      setSourceRuns((current: Record<string, SourceRunResult>) => ({
        ...current,
        ...Object.fromEntries(result.sources.map((source) => [source.id, source])),
      }))
    }
    const run = runRequest.mode === "watch"
      ? watchWorkspace(executionWorkspace, { onResult: applyResult })
      : runWorkspace(executionWorkspace, {
        dryRun: runRequest.mode === "list",
        sourceIds: listSourceId ? [listSourceId] : undefined,
      }).then(applyResult)
    void run
      .then(() => {
        if (controller.signal.aborted) {
          appActor.send({ type: "COMMAND", code: "app.run-stopped" })
        } else if (active && (runRequest.mode === "run" || runRequest.mode === "list")) {
          appActor.send({ type: "COMMAND", code: "app.run-complete" })
        }
      })
      .catch((error) => {
        if (controller.signal.aborted) {
          appActor.send({ type: "COMMAND", code: "app.run-stopped" })
        } else if (active) {
          setRunError(String(error))
          appActor.send({ type: "COMMAND", code: "app.run-failed" })
        }
      })

    return () => {
      active = false
      controller.abort()
    }
  }, [executableWorkspace, runRequest])

  useEffect(() => {
    if (exiting) renderer.destroy()
  }, [exiting, renderer])


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
          <box flexDirection="column" border={false} marginBottom={1} flexGrow={1} >
            <AppHeader />
            <box flexDirection="row" flexGrow={1} justifyContent="space-between" width="100%">
              <text>Workspace: {executableWorkspace.id}</text>
              <box flexDirection="row">
                <GlobalControl />
              </box>
            </box>
            <MainTabs />
          </box>
        }
        footer={< text > Ctrl + S Settings · Ctrl + R Refresh · Ctrl + Q Quit</text >}
      >
        <box flexDirection="column" border={false} marginBottom={1} flexGrow={1}>
          {runError ? <text fg={errorStyle.fg}>{runError}</text> : null}


          <box position="relative" flexGrow={1} padding={1}>
            {route.name === "workspace" ? <WorkspaceView workspace={executableWorkspace} sourceItems={sourceItems} /> : null}
            {route.name === "items" ? <ItemsView workspace={executableWorkspace} sourceItems={sourceItems} /> : null}
            {route.name === "source" ? (
              <SourceView
                key={`${route.sourceId}:${sourceItems.length}`}
                appActor={appActor}
                source={executableWorkspace.sources.find((source) => source.id === route.sourceId)!}
                items={sourceItems[route.sourceId] ?? []}
                runResult={sourceRuns[route.sourceId]}
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
            {route.name === "action-item" ? (
              <ActionItemView
                key={`${route.sourceId}:${route.itemId}:${route.actionIndex}`}
                appActor={appActor}
                source={executableWorkspace.sources.find((source) => source.id === route.sourceId)!}
                item={sourceItems[route.sourceId]?.find((item: Item) => item.id === route.itemId)!}
                actionIndex={route.actionIndex}
                runResult={sourceRuns[route.sourceId]}
              />
            ) : null}
            {settingsOpen ? <SettingsModal /> : null}
          </box>
        </box>
      </AppLayout >
    </KeymapScope >
  )
}

export function App(props: { workspacePath: string; version: string }) {
  const theme = loadTheme(props.workspacePath)

  return (
    <ThemeProvider theme={theme}>
      <AppMachineProvider workspacePath={props.workspacePath} version={props.version}>
        <KeymapProvider>
          <AppScreen />
        </KeymapProvider>
      </AppMachineProvider>
    </ThemeProvider>
  )
}

import {
  loadDataWorkspace,
  loadExecutableWorkspace,
  resolveWorkspaceConfigPath,
  type LoadedWorkspace,
} from "../services/config/workspace.ts";
import { runWorkspace, type WorkspaceRunResult } from "../services/runtime.ts";
import { app } from "./app.ts";
import { installCancellationHandlers } from "./cancellation.ts";

export function loadRunWorkspace(
  configPath: string,
  globalRoot?: string,
  cancellation: AbortSignal = new AbortController().signal,
): Promise<LoadedWorkspace> {
  const path = resolveWorkspaceConfigPath(configPath);
  return /agentboard\.config\.(ts|js)$/.test(path)
    ? loadExecutableWorkspace(path, undefined, cancellation)
    : loadDataWorkspace(path, globalRoot, cancellation);
}

export function runExitStatus(result: WorkspaceRunResult): number {
  if (result.cancelled) return 130;
  return result.sources.some((source) =>
      source.error !== undefined ||
      source.actions.some((action) =>
        action.error !== undefined || action.result?.outcome === "failure"
      )
    )
    ? 1
    : 0;
}

export const runCmd = app
  .sub("run")
  .args([{ name: "workspace", type: "string", default: ".agentboard.toml" }])
  .meta({ description: "Execute one workspace run" })
  .run(async ({ args }) => {
    const controller = new AbortController();
    const removeHandlers = installCancellationHandlers(controller);
    try {
      const workspace = await loadRunWorkspace(args.workspace, undefined, controller.signal);
      const result = await runWorkspace(workspace);
      console.log(JSON.stringify(result));
      process.exitCode = runExitStatus(result);
    } catch (error) {
      if (controller.signal.aborted) process.exitCode = 130;
      else throw error;
    } finally {
      removeHandlers();
    }
  });

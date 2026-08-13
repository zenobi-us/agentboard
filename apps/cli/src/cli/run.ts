import {
  loadDataWorkspace,
  loadExecutableWorkspace,
  resolveWorkspaceConfigPath,
  type LoadedWorkspace,
} from "../services/config/workspace.ts";
import { runWorkspace } from "../services/runtime.ts";
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
      if (result.cancelled) process.exitCode = 130;
    } catch (error) {
      if (controller.signal.aborted) process.exitCode = 130;
      else throw error;
    } finally {
      removeHandlers();
    }
  });

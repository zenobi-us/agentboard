import {
  loadDataWorkspace,
  loadExecutableWorkspace,
  resolveWorkspaceConfigPath,
  type LoadedWorkspace,
} from "../services/config/workspace.ts";
import { runWorkspace, watchWorkspace, type WorkspaceRunResult } from "../services/runtime.ts";
import { app } from "./app.ts";
import { installCancellationHandlers } from "./cancellation.ts";

export function loadRunWorkspace(
  configPath: string,
  globalRoot?: string,
  cancellation: AbortSignal = new AbortController().signal,
  prepareActions = true,
): Promise<LoadedWorkspace> {
  const path = resolveWorkspaceConfigPath(configPath);
  return /agentboard\.config\.(ts|js)$/.test(path)
    ? loadExecutableWorkspace(path, undefined, cancellation, prepareActions)
    : loadDataWorkspace(path, globalRoot, cancellation, prepareActions);
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

export function parseRunInterval(value: string): number {
  if (!/^\d+(?:\.\d+)?s?$/.test(value)) throw new TypeError("interval must be seconds");
  const milliseconds = Number(value.replace(/s$/, "")) * 1_000;
  if (milliseconds <= 0) throw new TypeError("interval must be greater than zero");
  return milliseconds;
}

export const runCmd = app
  .sub("run")
  .args([{ name: "workspace", type: "string", default: ".agentboard.toml" }])
  .flags({
    watch: { type: "boolean", description: "Repeat Runs until cancellation" },
    interval: { type: "string", default: "60s", parse: parseRunInterval },
  })
  .meta({ description: "Execute a Workspace Run" })
  .run(async ({ args, flags }) => {
    const controller = new AbortController();
    const removeHandlers = installCancellationHandlers(controller);
    try {
      const workspace = await loadRunWorkspace(args.workspace, undefined, controller.signal);
      const result = flags.watch
        ? await watchWorkspace(workspace, {
          intervalMs: flags.interval,
          onResult: (run) => console.log(JSON.stringify(run)),
        })
        : await runWorkspace(workspace);
      if (!flags.watch) console.log(JSON.stringify(result));
      process.exitCode = runExitStatus(result);
    } catch (error) {
      if (controller.signal.aborted) process.exitCode = 130;
      else throw error;
    } finally {
      removeHandlers();
    }
  });

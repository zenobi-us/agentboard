import { checkWorkspaceHealth } from "../services/runtime.ts";
import { app } from "./app.ts";
import { installCancellationHandlers } from "./cancellation.ts";
import { loadWorkspace } from "../services/workspace.ts";

export const doctorCmd = app
  .sub("doctor")
  .args([{ name: "workspace", type: "string", default: ".agentboard.toml" }])
  .meta({ description: "Validate the workspace and local environment" })
  .run(async ({ args }) => {
    const controller = new AbortController();
    const removeHandlers = installCancellationHandlers(controller);
    try {
      const results = await checkWorkspaceHealth(
        await loadWorkspace(args.workspace, undefined, controller.signal, false),
      );
      console.log(JSON.stringify(results));
      if (controller.signal.aborted) process.exitCode = 130;
      else if (results.some((result) => result.error)) process.exitCode = 1;
    } catch (error) {
      if (controller.signal.aborted) process.exitCode = 130;
      else throw error;
    } finally {
      removeHandlers();
    }
  });

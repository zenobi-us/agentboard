import { app } from "./app.ts";
import { loadRunWorkspace } from "./run.ts";
import { checkWorkspaceHealth } from "../services/runtime.ts";

export const doctorCmd = app
  .sub("doctor")
  .args([{ name: "workspace", type: "string", default: ".agentboard.toml" }])
  .meta({ description: "Validate the workspace and local environment" })
  .run(async ({ args }) => {
    const results = await checkWorkspaceHealth(
      await loadRunWorkspace(args.workspace),
    );
    console.log(JSON.stringify(results));
    if (results.some((result) => result.error)) process.exitCode = 1;
  });

import { app } from "./app.ts";
import { initializeWorkspace } from "./workspace.ts";

export const initCmd = app
  .sub("init")
  .args([{ name: "path", type: "string", default: ".clankpipe.toml" }])
  .meta({ description: "Create an empty Workspace" })
  .run(({ args }) => initializeWorkspace(args.path));

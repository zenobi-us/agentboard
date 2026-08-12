import {
  loadDataWorkspace,
  loadExecutableWorkspace,
  resolveWorkspaceConfigPath,
  type LoadedWorkspace,
} from "../services/config/workspace.ts";
import { app } from "./app.ts";

export function loadRunWorkspace(
  configPath: string,
  globalRoot?: string,
): Promise<LoadedWorkspace> {
  const path = resolveWorkspaceConfigPath(configPath);
  return /agentboard\.config\.(ts|js)$/.test(path)
    ? loadExecutableWorkspace(path)
    : loadDataWorkspace(path, globalRoot);
}

export const runCmd = app
  .sub("run")
  .args([{ name: "workspace", type: "string", default: ".agentboard.toml" }])
  .meta({ description: "Execute one workspace run" })
  .run(async ({ args }) => {
    const workspace = await loadRunWorkspace(args.workspace);
    console.log(`loaded ${workspace.sources.length} sources`);
  });

import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import {
  loadDataWorkspace,
  loadExecutableWorkspace,
  type LoadedWorkspace,
} from "../services/config/workspace.ts";
import { app } from "./app.ts";

export function loadRunWorkspace(
  configPath: string,
  globalRoot?: string,
): Promise<LoadedWorkspace> {
  const path = resolveRunConfigPath(configPath);
  return /agentboard\.config\.(ts|js)$/.test(path)
    ? loadExecutableWorkspace(path)
    : loadDataWorkspace(path, globalRoot);
}

function resolveRunConfigPath(configPath: string): string {
  if (configPath !== ".agentboard.toml") return configPath;
  const directory = dirname(configPath);
  for (const name of ["agentboard.config.ts", "agentboard.config.js"]) {
    const candidate = join(directory, name);
    if (existsSync(candidate)) return candidate;
  }
  return configPath;
}

export const runCmd = app
  .sub("run")
  .args([{ name: "workspace", type: "string", default: ".agentboard.toml" }])
  .meta({ description: "Execute one workspace run" })
  .run(async ({ args }) => {
    const workspace = await loadRunWorkspace(args.workspace);
    console.log(`loaded ${workspace.sources.length} sources`);
  });

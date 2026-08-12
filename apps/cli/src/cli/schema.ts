import { existsSync, statSync } from "node:fs";
import { dirname } from "node:path";
import {
  createWorkspaceSchemas,
  loadAllWorkspacePlugins,
} from "../services/config/workspace.ts";
import { app } from "./app.ts";

export async function loadSchemaWorkspace(configPath: string) {
  const directory = existsSync(configPath) && statSync(configPath).isFile()
    ? dirname(configPath)
    : configPath;
  const executable = ["agentboard.config.ts", "agentboard.config.js"]
    .map((name) => `${directory}/${name}`)
    .find(existsSync);
  return loadAllWorkspacePlugins(executable ?? configPath);
}

export const schemaCmd = app
  .sub("schema")
  .meta({ description: "Print the Workspace JSON Schema" })
  .run(async () => {
    const configPath = existsSync("agentboard.config.ts")
      ? "agentboard.config.ts"
      : existsSync("agentboard.config.js")
        ? "agentboard.config.js"
        : ".agentboard.toml";
    const registry = await loadSchemaWorkspace(configPath);
    console.log(JSON.stringify(createWorkspaceSchemas(registry).workspace, null, 2));
  });

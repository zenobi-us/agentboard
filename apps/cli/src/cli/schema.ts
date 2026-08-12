import {
  createWorkspaceSchemas,
  loadAllWorkspacePlugins,
  resolveWorkspaceConfigPath,
} from "../services/config/workspace.ts";
import { app } from "./app.ts";

export async function loadSchemaWorkspace(configPath: string) {
  return loadAllWorkspacePlugins(resolveWorkspaceConfigPath(configPath));
}

export const schemaCmd = app
  .sub("schema")
  .meta({ description: "Print the Workspace JSON Schema" })
  .run(async () => {
    const registry = await loadSchemaWorkspace(".agentboard.toml");
    console.log(JSON.stringify(createWorkspaceSchemas(registry).workspace, null, 2));
  });

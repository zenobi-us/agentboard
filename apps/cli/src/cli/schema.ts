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
  .args([{ name: "workspace", type: "string", default: ".agentboard.toml" }])
  .meta({ description: "Print the Workspace JSON Schema" })
  .run(async ({ args }) => {
    const registry = await loadSchemaWorkspace(args.workspace);
    console.log(JSON.stringify(createWorkspaceSchemas(registry).workspace, null, 2));
  });

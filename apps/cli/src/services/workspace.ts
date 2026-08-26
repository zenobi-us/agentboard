import {
  loadDataWorkspace,
  loadExecutableWorkspace,
  resolveWorkspaceConfigPath,
  type LoadedWorkspace,
} from "./config/workspace.ts";

export function loadWorkspace(
  configPath: string,
  globalRoot?: string,
  cancellation: AbortSignal = new AbortController().signal,
  createRuntimes = true,
): Promise<LoadedWorkspace> {
  const path = resolveWorkspaceConfigPath(configPath);
  return /agentboard\.config\.(ts|js)$/.test(path)
    ? loadExecutableWorkspace(path, undefined, cancellation, createRuntimes, globalRoot)
    : loadDataWorkspace(path, globalRoot, cancellation, createRuntimes);
}

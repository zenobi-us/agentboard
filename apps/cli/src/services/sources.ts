import {
  pluginFor,
  type ActionRuntime,
  type HealthCheckContext,
  type Item,
  type ResolvedSource,
  type SourceRuntime,
  type SourceRuntimeContext,
} from "@agentboard/core/config";
import type { TSchema } from "typebox";

import { createActionRuntime } from "./actions.ts";
import type { LoadedWorkspaceSource } from "./config/workspace.ts";

export interface LoadedSourceRuntime extends LoadedWorkspaceSource {
  readonly runtime: SourceRuntime;
  readonly actionFactories: readonly ((inputs: unknown) => ActionRuntime)[];
}

export async function createSourceRuntime(
  configured: LoadedWorkspaceSource,
  workspaceId: string,
): Promise<LoadedSourceRuntime> {
  const plugin = pluginFor(configured.source);
  if (plugin.kind !== "source") throw new TypeError("configuration node is not a Source");
  const context: SourceRuntimeContext = { sourceId: configured.id };
  const runtime = plugin.runtime(configured.source.config, context);
  if (!isSourceRuntime(runtime)) {
    throw new TypeError(`Source ${configured.id} runtime factory must return collect()`);
  }
  const actionFactories = configured.actions.map((action) => {
    const context = { workspaceId, sourceId: configured.id };
    createActionRuntime(action, action.config, context);
    return (inputs: unknown) => createActionRuntime(action, inputs, context);
  });
  return { ...configured, runtime, actionFactories };
}

export async function collectSource(source: LoadedSourceRuntime): Promise<readonly Item[]> {
  return await source.runtime.collect();
}

export async function checkSourceHealth(source: LoadedSourceRuntime): Promise<void> {
  const plugin = pluginFor(source.source as ResolvedSource<TSchema>);
  const context: HealthCheckContext = { sourceId: source.id };
  await plugin.healthCheck(source.source.config, context);
}

function isSourceRuntime(value: unknown): value is SourceRuntime {
  return value !== null && typeof value === "object" &&
    typeof (value as Partial<SourceRuntime>).collect === "function";
}

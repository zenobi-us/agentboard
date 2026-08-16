import {
  pluginFor,
  type HealthCheckContext,
  type Item,
  type ResolvedSource,
  type SourceRuntime,
  type SourceRuntimeContext,
} from "@agentboard/core/config";
import type { TSchema } from "typebox";

import type { LoadedWorkspaceSource } from "./config/workspace.ts";

export interface LoadedSourceRuntime extends LoadedWorkspaceSource {
  readonly runtime?: SourceRuntime;
  readonly cancellation: AbortSignal;
}

export async function createSourceRuntime(
  configured: LoadedWorkspaceSource,
  cancellation: AbortSignal,
): Promise<LoadedSourceRuntime> {
  const plugin = pluginFor(configured.source);
  if (plugin.kind !== "source") throw new TypeError("configuration node is not a Source");
  const context: SourceRuntimeContext = { sourceId: configured.id, cancellation };
  const runtime = await plugin.runtime(configured.source.config, context);
  if (!isSourceRuntime(runtime)) {
    throw new TypeError(`Source ${configured.id} runtime factory must return collect()`);
  }
  return { ...configured, runtime, cancellation };
}

export async function collectSource(source: LoadedSourceRuntime): Promise<readonly Item[]> {
  if (!source.runtime) throw new Error(`Source ${source.id} runtime is not available for a Run`);
  const items: unknown = await source.runtime.collect();
  if (!Array.isArray(items) || !items.every(isItem)) {
    throw new TypeError(`Source ${source.id} collect() must return normalized Items`);
  }
  return items;
}

export async function checkSourceHealth(source: LoadedSourceRuntime): Promise<void> {
  const plugin = pluginFor(source.source as ResolvedSource<TSchema>);
  const context: HealthCheckContext = {
    sourceId: source.id,
    cancellation: source.cancellation,
  };
  await plugin.healthCheck(source.source.config, context);
}

function isSourceRuntime(value: unknown): value is SourceRuntime {
  return value !== null && typeof value === "object" &&
    typeof (value as Partial<SourceRuntime>).collect === "function";
}

function isItem(value: unknown): value is Item {
  if (value === null || typeof value !== "object") return false;
  const item = value as Partial<Item>;
  return typeof item.id === "string" &&
    typeof item.reference_id === "string" &&
    typeof item.title === "string" &&
    typeof item.status === "string" &&
    typeof item.url === "string" &&
    typeof item.source_id === "string" &&
    typeof item.source_kind === "string" &&
    Object.hasOwn(item, "raw");
}

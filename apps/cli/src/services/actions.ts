import {
  pluginFor,
  type ActionResult,
  type ActionRuntime,
  type ActionRuntimeContext,
  type ActionRuntimeFactoryContext,
  type HealthCheckContext,
  type Item,
  type ResolvedAction,
} from "@agentboard/core/config";
import type { TSchema } from "typebox";

export async function executeAction(
  item: Item,
  runtime: ActionRuntime,
  context: ActionRuntimeFactoryContext,
): Promise<ActionResult> {
  const result: unknown = await runtime.execute({ ...context, item } satisfies ActionRuntimeContext);
  if (!isActionResult(result)) {
    throw new TypeError("Action runtime execute() must return an AgentBoard Action Result");
  }
  return result;
}

export function validateActionRuntime(
  configured: ResolvedAction<TSchema>,
  context: ActionRuntimeFactoryContext,
): void {
  const plugin = pluginFor(configured);
  if (plugin.kind !== "action") throw new TypeError("configuration node is not an Action");
  plugin.validateRuntime!(configured.config, context);
}

export function createActionRuntime(
  configured: ResolvedAction<TSchema>,
  inputs: unknown,
  context: ActionRuntimeFactoryContext,
): ActionRuntime {
  const plugin = pluginFor(configured);
  if (plugin.kind !== "action") throw new TypeError("configuration node is not an Action");
  const runtime = plugin.runtime(inputs, context);
  if (!isActionRuntime(runtime)) throw new TypeError("Action runtime factory must return execute()");
  return runtime;
}

export async function checkActionHealth(
  sourceId: string,
  configured: ResolvedAction<TSchema>,
  cancellation: AbortSignal,
): Promise<void> {
  const plugin = pluginFor(configured);
  const context: HealthCheckContext = { sourceId, cancellation };
  await plugin.healthCheck(configured.config, context);
}

function isActionRuntime(value: unknown): value is ActionRuntime {
  return value !== null && typeof value === "object" &&
    typeof (value as Partial<ActionRuntime>).execute === "function";
}

function isActionResult(value: unknown): value is ActionResult {
  if (value === null || typeof value !== "object") return false;
  const result = value as Partial<ActionResult>;
  return (result.outcome === "success" || result.outcome === "failure" || result.outcome === "cancelled") &&
    typeof result.stdout === "string" &&
    typeof result.stderr === "string" &&
    (result.message === undefined || typeof result.message === "string");
}

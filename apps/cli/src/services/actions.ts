import {
  pluginFor,
  type ActionResult,
  type ActionRuntime,
  type ActionRuntimeContext,
  type ActionRuntimeFactoryContext,
  type HealthCheckContext,
  type Item,
  type PreparedActionRuntime,
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

export function prepareActionRuntime(
  configured: ResolvedAction<TSchema>,
  context: ActionRuntimeFactoryContext,
): PreparedActionRuntime {
  const plugin = pluginFor(configured);
  if (plugin.kind !== "action") throw new TypeError("configuration node is not an Action");
  try {
    const prepared = plugin.runtime(configured.config, context);
    if (!isPreparedActionRuntime(prepared)) throw new TypeError("must return create()");
    return prepared;
  } catch (error) {
    throw new Error(`Action runtime preparation failed: ${errorMessage(error)}`);
  }
}

export class ActionRuntimeFactoryError extends Error {}

export function createActionRuntime(
  prepared: PreparedActionRuntime,
  inputs: unknown,
): ActionRuntime {
  try {
    const runtime = prepared.create(inputs);
    if (!isActionRuntime(runtime)) throw new TypeError("must return execute()");
    return runtime;
  } catch (error) {
    throw new ActionRuntimeFactoryError(`Action runtime factory failed: ${errorMessage(error)}`);
  }
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

function isPreparedActionRuntime(value: unknown): value is PreparedActionRuntime {
  return value !== null && typeof value === "object" &&
    typeof (value as Partial<PreparedActionRuntime>).create === "function";
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function isActionResult(value: unknown): value is ActionResult {
  if (value === null || typeof value !== "object") return false;
  const result = value as Partial<ActionResult>;
  return (result.outcome === "success" || result.outcome === "failure" || result.outcome === "cancelled") &&
    typeof result.stdout === "string" &&
    typeof result.stderr === "string" &&
    (result.message === undefined || typeof result.message === "string");
}

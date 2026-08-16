import {
  pluginFor,
  type ActionResult,
  type ActionRuntime,
  type ActionRuntimeContext,
  type ActionPreparationContext,
  type HealthCheckContext,
  type Item,
  type PreparedAction,
  type ResolvedAction,
} from "@agentboard/core/config";
import type { TSchema } from "typebox";

export async function executeAction(
  item: Item,
  runtime: ActionRuntime,
  context: ActionPreparationContext,
): Promise<ActionResult> {
  const result: unknown = await runtime.execute({ ...context, item } satisfies ActionRuntimeContext);
  if (!isActionResult(result)) {
    throw new TypeError("Action runtime execute() must return an AgentBoard Action Result");
  }
  return result;
}

export async function prepareAction(
  configured: ResolvedAction<TSchema>,
  context: ActionPreparationContext,
): Promise<PreparedAction> {
  const plugin = pluginFor(configured);
  if (plugin.kind !== "action") throw new TypeError("configuration node is not an Action");
  try {
    const prepared = await plugin.prepare(configured.config, context);
    if (!isPreparedAction(prepared)) throw new TypeError("must return create()");
    return prepared;
  } catch (error) {
    throw new Error(`Action preparation failed: ${errorMessage(error)}`);
  }
}

export function createActionRuntime(
  prepared: PreparedAction,
  inputs: unknown,
): ActionRuntime {
  try {
    const runtime = prepared.create(inputs);
    if (!isActionRuntime(runtime)) throw new TypeError("must return execute()");
    return runtime;
  } catch (error) {
    throw new Error(`Action runtime creation failed: ${errorMessage(error)}`);
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

function isPreparedAction(value: unknown): value is PreparedAction {
  return value !== null && typeof value === "object" &&
    typeof (value as Partial<PreparedAction>).create === "function";
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

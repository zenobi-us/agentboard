import {
  pluginFor,
  type ActionResult,
  type ActionRuntime,
  type ActionRuntimeCreationContext,
  type HealthCheckContext,
  type Item,
  type ResolvedAction,
} from "@agentboard/core/config";
import type { TSchema } from "typebox";

export async function executeAction(
  item: Item,
  inputs: unknown,
  runtime: ActionRuntime,
  context: ActionRuntimeCreationContext,
): Promise<ActionResult> {
  const result: unknown = await runtime.execute({ ...context, item, inputs });
  if (!isActionResult(result)) {
    throw new TypeError("Action runtime execute() must return an AgentBoard Action Result");
  }
  return result;
}

export async function createActionRuntime(
  configured: ResolvedAction<TSchema>,
  context: ActionRuntimeCreationContext,
): Promise<ActionRuntime> {
  const plugin = pluginFor(configured);
  if (plugin.kind !== "action") throw new TypeError("configuration node is not an Action");
  try {
    const runtime = await plugin.runtime(configured.config, context);
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


function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function isActionResult(value: unknown): value is ActionResult {
  if (value === null || typeof value !== "object") return false;
  if (Object.keys(value).some((key) => !["outcome", "stdout", "stderr", "message"].includes(key))) {
    return false;
  }
  const result = value as Partial<ActionResult>;
  return (result.outcome === "success" || result.outcome === "failure" || result.outcome === "cancelled") &&
    typeof result.stdout === "string" &&
    typeof result.stderr === "string" &&
    (result.message === undefined || typeof result.message === "string");
}

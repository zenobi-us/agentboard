import {
  pluginFor,
  type ActionResult,
  type Item,
} from "@agentboard/core/config";

import type { LoadedWorkspace } from "./config/workspace.ts";
import {
  checkActionHealth,
  createActionRuntime,
  executeAction,
} from "./actions.ts";
import { checkSourceHealth, collectSource } from "./sources.ts";
import {
  acquireWorkspaceLock,
  actionKey,
  appendActionAttempt,
  appendSourceSnapshot,
  renderedActionHash,
  setSourceCollectionStatus,
  successfulActionKeys,
} from "./store.ts";
import { renderActionInputs } from "./template.ts";

export interface ActionRunResult {
  readonly itemId: string;
  readonly actionIndex: number;
  readonly uses: string;
  readonly result?: ActionResult;
  readonly error?: string;
  readonly skipped?: boolean;
}

export interface SourceRunResult {
  readonly id: string;
  readonly uses: string;
  readonly items: readonly Item[];
  readonly actions: readonly ActionRunResult[];
  readonly error?: string;
}

export interface WorkspaceRunResult {
  readonly sources: readonly SourceRunResult[];
  readonly cancelled?: boolean;
}

export interface RunWorkspaceOptions {
  readonly storeRoot?: string;
}

export interface WatchWorkspaceOptions extends RunWorkspaceOptions {
  readonly intervalMs?: number;
  readonly onResult?: (result: WorkspaceRunResult) => void;
}

export interface HealthCheckResult {
  readonly sourceId: string;
  readonly role: "source" | "action";
  readonly uses: string;
  readonly actionIndex?: number;
  readonly error?: string;
}

export async function checkWorkspaceHealth(
  workspace: LoadedWorkspace,
): Promise<readonly HealthCheckResult[]> {
  const results: HealthCheckResult[] = [];
  for (const source of workspace.sources) {
    if (source.cancellation.aborted) break;
    try {
      await checkSourceHealth(source);
      results.push({ sourceId: source.id, role: "source", uses: source.packageName });
    } catch (error) {
      results.push({
        sourceId: source.id,
        role: "source",
        uses: source.packageName,
        error: errorMessage(error),
      });
    }
    for (const [actionIndex, action] of source.actions.entries()) {
      if (source.cancellation.aborted) break;
      try {
        await checkActionHealth(source.id, action, source.cancellation);
        results.push({
          sourceId: source.id,
          role: "action",
          uses: action.packageName,
          actionIndex,
        });
      } catch (error) {
        results.push({
          sourceId: source.id,
          role: "action",
          uses: action.packageName,
          actionIndex,
          error: errorMessage(error),
        });
      }
    }
  }
  return results;
}

export async function runWorkspace(
  workspace: LoadedWorkspace,
  options: RunWorkspaceOptions = {},
): Promise<WorkspaceRunResult> {
  const releaseLock = await acquireWorkspaceLock(workspace, options.storeRoot);
  try {
    return await runWorkspaceUnlocked(workspace, options.storeRoot);
  } finally {
    await releaseLock();
  }
}

export async function watchWorkspace(
  workspace: LoadedWorkspace,
  options: WatchWorkspaceOptions = {},
): Promise<WorkspaceRunResult> {
  const releaseLock = await acquireWorkspaceLock(workspace, options.storeRoot);
  try {
    let result: WorkspaceRunResult;
    do {
      result = await runWorkspaceUnlocked(workspace, options.storeRoot);
      options.onResult?.(result);
      if (!result.cancelled && !workspace.cancellation.aborted) {
        await waitForNextRun(workspace.cancellation, options.intervalMs ?? 60_000);
      }
    } while (!result.cancelled && !workspace.cancellation.aborted);
    return result;
  } finally {
    await releaseLock();
  }
}

async function runWorkspaceUnlocked(
  workspace: LoadedWorkspace,
  storeRoot?: string,
): Promise<WorkspaceRunResult> {
  const settled = await Promise.allSettled(workspace.sources.map((source) =>
    runSource(workspace, source, storeRoot)
  ));
  const sources = settled.map((result) => {
    if (result.status === "rejected") throw result.reason;
    return result.value;
  });
  return {
    sources: sources.map(({ result }) => result),
    ...(sources.some(({ cancelled }) => cancelled) ? { cancelled: true } : {}),
  };
}

async function waitForNextRun(cancellation: AbortSignal, intervalMs: number): Promise<void> {
  await new Promise<void>((resolve) => {
    const done = () => {
      clearTimeout(timeout);
      cancellation.removeEventListener("abort", done);
      resolve();
    };
    const timeout = setTimeout(done, intervalMs);
    cancellation.addEventListener("abort", done, { once: true });
  });
}

async function runSource(
  workspace: LoadedWorkspace,
  source: LoadedWorkspace["sources"][number],
  storeRoot?: string,
): Promise<{ readonly result: SourceRunResult; readonly cancelled: boolean }> {
  if (source.cancellation.aborted) {
    return {
      result: { id: source.id, uses: source.packageName, items: [], actions: [] },
      cancelled: true,
    };
  }
  await setSourceCollectionStatus(workspace, source.id, "collecting", undefined, storeRoot);
  let items: readonly Item[];
  try {
    items = await collectSource(source);
    if (source.cancellation.aborted) throw source.cancellation.reason ?? new Error("cancelled");
    await appendSourceSnapshot(workspace, source, items, source.cancellation, storeRoot);
    await setSourceCollectionStatus(workspace, source.id, "complete", undefined, storeRoot);
  } catch (error) {
    if (source.cancellation.aborted) {
      await setSourceCollectionStatus(workspace, source.id, "cancelled", undefined, storeRoot);
      return {
        result: { id: source.id, uses: source.packageName, items: [], actions: [] },
        cancelled: true,
      };
    }
    const message = errorMessage(error);
    await setSourceCollectionStatus(workspace, source.id, "failed", message, storeRoot);
    return {
      result: { id: source.id, uses: source.packageName, items: [], actions: [], error: message },
      cancelled: false,
    };
  }
  const actions = await runActions(workspace, source, items, storeRoot);
  return {
    result: { id: source.id, uses: source.packageName, items, actions: actions.results },
    cancelled: actions.cancelled,
  };
}

async function runActions(
  workspace: LoadedWorkspace,
  source: LoadedWorkspace["sources"][number],
  items: readonly Item[],
  storeRoot?: string,
): Promise<{ readonly results: ActionRunResult[]; readonly cancelled: boolean }> {
  const results: ActionRunResult[] = [];
  const successes = await successfulActionKeys(workspace, source, storeRoot);
  for (const item of items) {
    const actions: Record<string, { inputs: unknown }> = {};
    for (const [actionIndex, action] of source.actions.entries()) {
      let renderedHash = "";
      try {
        const inputs = renderActionInputs(action.config, {
          workspace: { id: workspace.id, path: workspace.path },
          source: {
            id: source.id,
            source: Object.assign(
              {
                kind: source.source.config !== null &&
                    typeof source.source.config === "object" &&
                    typeof (source.source.config as Record<string, unknown>)["kind"] === "string"
                  ? (source.source.config as Record<string, unknown>)["kind"]
                  : source.packageName,
                uses: source.packageName,
              },
              source.source.config !== null && typeof source.source.config === "object"
                ? source.source.config
                : { value: source.source.config },
            ),
            actions: source.actions.map((configured) => ({
              id: configured.id,
              uses: configured.packageName,
              with: configured.config,
            })),
          },
          item,
          action: { index: actionIndex, uses: action.packageName },
          actions,
        }, { pathInputs: pluginPathInputs(action) });
        if (action.id) actions[action.id] = { inputs };
        renderedHash = renderedActionHash(action.packageName, inputs);
        if (source.cancellation.aborted) return { results, cancelled: true };
        const context = {
          workspaceId: workspace.id,
          sourceId: source.id,
          cancellation: source.cancellation,
        };
        if (!action.preparedAction) throw new Error("Action was not prepared for a Run");
        const runtime = createActionRuntime(action.preparedAction, inputs);
        if (
          successes.has(actionKey(source.id, item.id, actionIndex, renderedHash)) &&
          await (runtime.cachedSuccessIsValid?.({ ...context, item }) ?? true)
        ) {
          if (source.cancellation.aborted) return { results, cancelled: true };
          results.push({ itemId: item.id, actionIndex, uses: action.packageName, skipped: true });
          continue;
        }
        if (source.cancellation.aborted) return { results, cancelled: true };
        const result = await executeAction(item, runtime, context);
        await appendActionAttempt(workspace, source, {
          ts: new Date().toISOString(),
          source_id: source.id,
          item_id: item.id,
          source_action_index: actionIndex,
          uses: action.packageName,
          rendered_action_hash: renderedHash,
          ...result,
        }, storeRoot);
        results.push({ itemId: item.id, actionIndex, uses: action.packageName, result });
        if (result.outcome === "cancelled") return { results, cancelled: true };
        if (result.outcome === "failure") break;
        successes.add(actionKey(source.id, item.id, actionIndex, renderedHash));
      } catch (error) {
        const cancelled = source.cancellation.aborted;
        const message = errorMessage(error);
        await appendActionAttempt(workspace, source, {
          ts: new Date().toISOString(),
          source_id: source.id,
          item_id: item.id,
          source_action_index: actionIndex,
          uses: action.packageName,
          rendered_action_hash: renderedHash,
          outcome: cancelled ? "cancelled" : "failure",
          stdout: "",
          stderr: cancelled ? "" : message,
          message,
        }, storeRoot);
        results.push(cancelled
          ? {
              itemId: item.id,
              actionIndex,
              uses: action.packageName,
              result: { outcome: "cancelled", stdout: "", stderr: "", message },
            }
          : { itemId: item.id, actionIndex, uses: action.packageName, error: message });
        if (cancelled) return { results, cancelled: true };
        break;
      }
    }
  }
  return { results, cancelled: false };
}

function pluginPathInputs(
  action: LoadedWorkspace["sources"][number]["actions"][number],
): readonly string[] {
  return pluginFor(action).pathInputs ?? [];
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

import {
  pluginFor,
  type ActionResult,
  type Item,
} from "@agentboard/core/config";

import type { LoadedWorkspace } from "./config/workspace.ts";
import {
  checkActionHealth,
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
  readonly dryRun?: boolean;
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
  if (options.dryRun) return runWorkspaceUnlocked(workspace, options.storeRoot, true);
  const releaseLock = await acquireWorkspaceLock(workspace, options.storeRoot);
  try {
    return await runWorkspaceUnlocked(workspace, options.storeRoot, false);
  } finally {
    await releaseLock();
  }
}

export async function watchWorkspace(
  workspace: LoadedWorkspace,
  options: WatchWorkspaceOptions = {},
): Promise<WorkspaceRunResult> {
  if (options.dryRun) return runWorkspaceUnlocked(workspace, options.storeRoot, true);
  const releaseLock = await acquireWorkspaceLock(workspace, options.storeRoot);
  try {
    let result: WorkspaceRunResult;
    do {
      result = await runWorkspaceUnlocked(workspace, options.storeRoot, options.dryRun);
      options.onResult?.(result);
      if (!result.cancelled && !workspace.cancellation.aborted) {
        if (await waitForNextRun(workspace.cancellation, options.intervalMs ?? 60_000)) {
          return { ...result, cancelled: true };
        }
      }
    } while (!result.cancelled && !workspace.cancellation.aborted);
    return workspace.cancellation.aborted && !result.cancelled
      ? { ...result, cancelled: true }
      : result;
  } finally {
    await releaseLock();
  }
}

async function runWorkspaceUnlocked(
  workspace: LoadedWorkspace,
  storeRoot?: string,
  dryRun = false,
): Promise<WorkspaceRunResult> {
  const settled = await Promise.allSettled(workspace.sources.map((source) =>
    runSource(workspace, source, storeRoot, dryRun)
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

async function waitForNextRun(cancellation: AbortSignal, intervalMs: number): Promise<boolean> {
  if (cancellation.aborted) return true;
  return await new Promise<boolean>((resolve) => {
    let settled = false;
    const done = () => {
      if (settled) return;
      settled = true;
      clearTimeout(timeout);
      cancellation.removeEventListener("abort", done);
      resolve(cancellation.aborted);
    };
    const timeout = setTimeout(done, intervalMs);
    cancellation.addEventListener("abort", done, { once: true });
    if (cancellation.aborted) done();
  });
}

async function runSource(
  workspace: LoadedWorkspace,
  source: LoadedWorkspace["sources"][number],
  storeRoot?: string,
  dryRun = false,
): Promise<{ readonly result: SourceRunResult; readonly cancelled: boolean }> {
  if (source.cancellation.aborted) {
    return {
      result: { id: source.id, uses: source.packageName, items: [], actions: [] },
      cancelled: true,
    };
  }
  if (!dryRun) await setSourceCollectionStatus(workspace, source.id, "collecting", undefined, storeRoot);
  if (source.cancellation.aborted) {
    if (!dryRun) await setSourceCollectionStatus(workspace, source.id, "cancelled", undefined, storeRoot);
    return {
      result: { id: source.id, uses: source.packageName, items: [], actions: [] },
      cancelled: true,
    };
  }
  let items: readonly Item[];
  try {
    items = await collectSource(source);
    if (source.cancellation.aborted) throw source.cancellation.reason ?? new Error("cancelled");
    if (!dryRun) {
      await appendSourceSnapshot(workspace, source, items, source.cancellation, storeRoot);
      await setSourceCollectionStatus(workspace, source.id, "complete", undefined, storeRoot);
    }
    if (source.cancellation.aborted && source.actions.length === 0) {
      return {
        result: { id: source.id, uses: source.packageName, items, actions: [] },
        cancelled: false,
      };
    }
  } catch (error) {
    if (source.cancellation.aborted) {
      if (!dryRun) await setSourceCollectionStatus(workspace, source.id, "cancelled", undefined, storeRoot);
      return {
        result: { id: source.id, uses: source.packageName, items: [], actions: [] },
        cancelled: true,
      };
    }
    const message = errorMessage(error);
    if (!dryRun) await setSourceCollectionStatus(workspace, source.id, "failed", message, storeRoot);
    return {
      result: { id: source.id, uses: source.packageName, items: [], actions: [], error: message },
      cancelled: false,
    };
  }
  const actions = await runActions(workspace, source, items, storeRoot, dryRun);
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
  dryRun = false,
): Promise<{ readonly results: ActionRunResult[]; readonly cancelled: boolean }> {
  const results: ActionRunResult[] = [];
  const successes = await successfulActionKeys(workspace, source, storeRoot);
  if (source.cancellation.aborted) return { results, cancelled: true };
  for (const item of items) {
    const actions: Record<string, { inputs: unknown }> = {};
    for (const [actionIndex, action] of source.actions.entries()) {
      let renderedHash = "";
      try {
        const inputs = renderActionInputs(action.config, {
          workspace: { id: workspace.id, path: workspace.path },
          source: {
            id: source.id,
            source: {
              uses: source.packageName,
              ...(source.source.config !== null && typeof source.source.config === "object"
                ? source.source.config
                : { value: source.source.config }),
            },
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
        if (dryRun) {
          results.push({ itemId: item.id, actionIndex, uses: action.packageName, skipped: true });
          continue;
        }
        const context = {
          workspaceId: workspace.id,
          sourceId: source.id,
          cancellation: source.cancellation,
        };
        if (!action.runtime) throw new Error("Action runtime is not available for a Run");
        const executionContext = { ...context, item, inputs };
        if (
          successes.has(actionKey(source.id, item.id, actionIndex, renderedHash)) &&
          await (action.runtime.cachedSuccessIsValid?.(executionContext) ?? true)
        ) {
          if (source.cancellation.aborted) return { results, cancelled: true };
          results.push({ itemId: item.id, actionIndex, uses: action.packageName, skipped: true });
          continue;
        }
        if (source.cancellation.aborted) return { results, cancelled: true };
        const result = await executeAction(item, inputs, action.runtime, context);
        const persistedResult = source.cancellation.aborted && result.outcome !== "cancelled"
          ? { ...result, outcome: "cancelled" as const, message: "action cancelled" }
          : result;
        await appendActionAttempt(workspace, source, {
          ts: new Date().toISOString(),
          ...persistedResult,
          source_id: source.id,
          item_id: item.id,
          source_action_index: actionIndex,
          uses: action.packageName,
          rendered_action_hash: renderedHash,
        }, storeRoot);
        const finalResult = source.cancellation.aborted && persistedResult.outcome !== "cancelled"
          ? { ...persistedResult, outcome: "cancelled" as const, message: "action cancelled" }
          : persistedResult;
        if (finalResult !== persistedResult) {
          await appendActionAttempt(workspace, source, {
            ts: new Date().toISOString(),
            ...finalResult,
            outcome: "cancelled",
            message: "action cancelled",
            source_id: source.id,
            item_id: item.id,
            source_action_index: actionIndex,
            uses: action.packageName,
            rendered_action_hash: renderedHash,
          }, storeRoot);
        }
        results.push({ itemId: item.id, actionIndex, uses: action.packageName, result: finalResult });
        if (source.cancellation.aborted || finalResult.outcome === "cancelled") {
          return { results, cancelled: true };
        }
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
        if (source.cancellation.aborted) {
          results.push({ itemId: item.id, actionIndex, uses: action.packageName, error: message });
          return { results, cancelled: true };
        }
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

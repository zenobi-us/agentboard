import type {
  ActionResult,
  Item,
} from "@agentboard/core/config";

import type { LoadedWorkspace } from "./config/workspace.ts";
import { checkActionHealth, createActionRuntime, executeAction } from "./actions.ts";
import { checkSourceHealth, collectSource } from "./sources.ts";
import {
  acquireWorkspaceLock,
  actionKey,
  appendActionAttempt,
  appendSourceSnapshot,
  renderedActionHash,
  successfulActionKeys,
} from "./store.ts";
import { renderActionInputs } from "./template.ts";

export interface ActionRunResult {
  readonly itemId: string;
  readonly actionIndex: number;
  readonly result?: ActionResult;
  readonly error?: string;
  readonly skipped?: boolean;
}

export interface SourceRunResult {
  readonly id: string;
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

export interface HealthCheckResult {
  readonly sourceId: string;
  readonly role: "source" | "action";
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
      results.push({ sourceId: source.id, role: "source" });
    } catch (error) {
      results.push({ sourceId: source.id, role: "source", error: errorMessage(error) });
    }
    for (const [actionIndex, action] of source.actions.entries()) {
      if (source.cancellation.aborted) break;
      try {
        await checkActionHealth(source.id, action, source.cancellation);
        results.push({ sourceId: source.id, role: "action", actionIndex });
      } catch (error) {
        results.push({
          sourceId: source.id,
          role: "action",
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
  const sources: SourceRunResult[] = [];
  const releaseLock = await acquireWorkspaceLock(workspace, options.storeRoot);
  try {
    for (const source of workspace.sources) {
      try {
        if (source.cancellation.aborted) return { sources, cancelled: true };
        const items = await collectSource(source);
        if (source.cancellation.aborted) return { sources, cancelled: true };
        await appendSourceSnapshot(
          workspace,
          source,
          items,
          source.cancellation,
          options.storeRoot,
        );
        const actions = await runActions(workspace, source, items, options.storeRoot);
        sources.push({ id: source.id, items, actions: actions.results });
        if (actions.cancelled) return { sources, cancelled: true };
      } catch (error) {
        if (source.cancellation.aborted) return { sources, cancelled: true };
        sources.push({
          id: source.id,
          items: [],
          actions: [],
          error: errorMessage(error),
        });
      }
    }
    return { sources };
  } finally {
    await releaseLock();
  }
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
        }, { pathInputs: actionPathInputs(action.packageName) });
        if (action.id) actions[action.id] = { inputs };
        renderedHash = renderedActionHash(action.packageName, inputs);
        if (source.cancellation.aborted) return { results, cancelled: true };
        const context = {
          workspaceId: workspace.id,
          sourceId: source.id,
          cancellation: source.cancellation,
        };
        const runtime = createActionRuntime(action, inputs, context);
        if (successes.has(actionKey(source.id, item.id, actionIndex, renderedHash))) {
          results.push({ itemId: item.id, actionIndex, skipped: true });
          continue;
        }
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
        results.push({ itemId: item.id, actionIndex, result });
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
              result: { outcome: "cancelled", stdout: "", stderr: "", message },
            }
          : { itemId: item.id, actionIndex, error: message });
        if (cancelled) return { results, cancelled: true };
        break;
      }
    }
  }
  return { results, cancelled: false };
}

function actionPathInputs(packageName: string): readonly string[] {
  if (
    packageName === "agentboard/run-cmd" ||
    packageName === "@agentboard/action-run-cmd" ||
    packageName.includes("agentboard-action-run-cmd")
  ) {
    return ["cwd"];
  }
  if (
    packageName === "agentboard/worktree" ||
    packageName === "@agentboard/action-worktree" ||
    packageName.includes("agentboard-action-worktree")
  ) {
    return ["repo", "root"];
  }
  return [];
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

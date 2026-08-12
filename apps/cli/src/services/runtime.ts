import type {
  ActionResult,
  Item,
} from "@agentboard/core/config";

import type { LoadedWorkspace } from "./config/workspace.ts";
import { checkActionHealth, executeAction } from "./actions.ts";
import { checkSourceHealth, collectSource } from "./sources.ts";
import { renderActionInputs } from "./template.ts";

export interface ActionRunResult {
  readonly itemId: string;
  readonly actionIndex: number;
  readonly result?: ActionResult;
  readonly error?: string;
}

export interface SourceRunResult {
  readonly id: string;
  readonly items: readonly Item[];
  readonly actions: readonly ActionRunResult[];
  readonly error?: string;
}

export interface WorkspaceRunResult {
  readonly sources: readonly SourceRunResult[];
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
    try {
      await checkSourceHealth(source);
      results.push({ sourceId: source.id, role: "source" });
    } catch (error) {
      results.push({ sourceId: source.id, role: "source", error: errorMessage(error) });
    }
    for (const [actionIndex, action] of source.actions.entries()) {
      try {
        await checkActionHealth(source.id, action);
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

export async function runWorkspace(workspace: LoadedWorkspace): Promise<WorkspaceRunResult> {
  const sources: SourceRunResult[] = [];
  for (const source of workspace.sources) {
    try {
      const items = await collectSource(source);
      sources.push({
        id: source.id,
        items,
        actions: await runActions(workspace.id, source, items),
      });
    } catch (error) {
      sources.push({
        id: source.id,
        items: [],
        actions: [],
        error: errorMessage(error),
      });
    }
  }
  return { sources };
}

async function runActions(
  workspaceId: string,
  source: LoadedWorkspace["sources"][number],
  items: readonly Item[],
): Promise<ActionRunResult[]> {
  const results: ActionRunResult[] = [];
  for (const item of items) {
    const actions: Record<string, { inputs: unknown }> = {};
    for (const [actionIndex, action] of source.actions.entries()) {
      try {
        const inputs = renderActionInputs(action.config, {
          workspace: { id: workspaceId, path: source.source.identity.path },
          source: {
            id: source.id,
            source: Object.assign(
              { uses: source.packageName },
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
        });
        if (action.id) actions[action.id] = { inputs };
        results.push({
          itemId: item.id,
          actionIndex,
          result: await executeAction(
            item,
            source.actionFactories[actionIndex]!(inputs),
            { workspaceId, sourceId: source.id },
          ),
        });
      } catch (error) {
        results.push({ itemId: item.id, actionIndex, error: errorMessage(error) });
        break;
      }
    }
  }
  return results;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

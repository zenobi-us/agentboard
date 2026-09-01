import { app, watchView } from "./app.ts";
import { parseRunInterval } from "./run.ts";
import { installCancellationHandlers } from "./cancellation.ts";
import { loadWorkspace } from "../services/workspace.ts";
import { readStoredItems, readStoreViews } from "../services/store.ts";

export async function storedItems(workspacePath: string): Promise<unknown[]> {
  const workspace = await loadWorkspace(workspacePath, undefined, undefined, false);
  return (await readStoredItems(workspace)).map(({ item, sourceSlug, actionState }) => ({
    item,
    action_state: actionState,
    source_slug: sourceSlug,
  }));
}

async function renderList(workspace: Awaited<ReturnType<typeof loadWorkspace>>, asJson: boolean): Promise<string> {
  if (asJson) {
    const snapshots = await readStoreViews(workspace);
    return `${JSON.stringify(snapshots.map(({ sourceId, state, items, pipeline, collectionStatus }) => ({
      source_id: sourceId,
      snapshot: state,
      collection_status: collectionStatus,
      pipeline: (pipeline ?? []).map(({ item, state: pipelineState }) => ({ item, state: pipelineState })),
      items: items.map(({ item, actionState }) => ({
        item,
        result: actionState === "succeeded" ? "success" : actionState === "failed" ? "error" : "pending",
      })),
    })), null, 2)}\n`;
  }
  return renderListHuman(await readStoreViews(workspace));
}

export function renderListHuman(snapshots: Awaited<ReturnType<typeof readStoreViews>>): string {
  const lines: string[] = [];
  for (const snapshot of snapshots) {
    lines.push(`Source: ${snapshot.sourceId}`);
    if (snapshot.state === "missing") {
      lines.push("Snapshot: missing (run successfully to populate it)");
    } else if (snapshot.items.length === 0) {
      lines.push("Snapshot: ready (0 items)");
    } else {
      lines.push("Reference ID\tTitle\tStatus\tAction Plan Result");
      lines.push(...snapshot.items.map(({ item, actionState }) => `${item.reference_id}\t${item.title}\t${item.status}\t${actionState === "succeeded" ? "success" : actionState === "failed" ? "error" : "pending"}`));
    }
    for (const execution of snapshot.pipeline ?? []) {
      lines.push(`Pipeline\t${execution.state}\t${execution.item.reference_id}\t${execution.item.title}`);
    }
    if (snapshot.collectionStatus?.error) lines.splice(-1, 0, `Collection error: ${snapshot.collectionStatus.error}`);
    lines.push("");
  }
  return lines.length > 0 ? `${lines.join("\n")}\n` : "";
}

export const listCmd = app
  .sub("list")
  .args([{ name: "workspace", type: "string", default: ".agentboard.toml" }])
  .flags({
    json: { type: "boolean", description: "Print JSON" },
    watch: { type: "boolean", description: "Refresh until cancellation" },
    interval: { type: "string", default: "60s", parse: parseRunInterval },
  })
  .meta({ description: "List the latest stored items" })
  .run(async ({ args, flags }) => {
    const controller = new AbortController();
    const removeHandlers = installCancellationHandlers(controller);
    try {
      const workspace = await loadWorkspace(args.workspace, undefined, controller.signal, false);
      if (flags.watch && flags.json) throw new Error("--watch cannot be combined with --json");
      if (flags.watch) {
        await watchView("list", flags.interval, () => renderList(workspace, false), controller.signal);
      } else {
        process.stdout.write(await renderList(workspace, flags.json === true));
      }
    } finally {
      removeHandlers();
    }
  });

export async function storedSnapshots(workspacePath: string) {
  const workspace = await loadWorkspace(workspacePath, undefined, undefined, false);
  return readStoreViews(workspace);
}

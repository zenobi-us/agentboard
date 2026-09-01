import { app, watchView } from "./app.ts";
import { parseRunInterval } from "./run.ts";
import { installCancellationHandlers } from "./cancellation.ts";
import { loadWorkspace } from "../services/workspace.ts";
import { readStoredItems } from "../services/store.ts";

async function renderShow(workspace: Awaited<ReturnType<typeof loadWorkspace>>, itemRef: string, asJson: boolean): Promise<string> {
  const value = (await readStoredItems(workspace)).find(({ item }) => item.id === itemRef || item.reference_id === itemRef);
  if (!value) throw new Error(`item ${itemRef} not found`);
  if (asJson) return `${JSON.stringify({ source_slug: value.sourceSlug, item: value.item, actions: value.actions }, null, 2)}\n`;
  return `${value.item.id}\n${value.item.title}\n${value.item.status}\n${value.item.url}\n${value.actions.map((action) => `action#${action.source_action_index} ${action.uses} outcome=${action.outcome}`).join("\n")}${value.actions.length ? "\n" : ""}`;
}

export const showCmd = app
  .sub("show")
  .args([
    { name: "workspace", type: "string", default: ".clankpipe.toml" },
    { name: "item", type: "string" },
  ])
  .flags({
    json: { type: "boolean", description: "Print JSON" },
    watch: { type: "boolean", description: "Refresh until cancellation" },
    interval: { type: "string", default: "60s", parse: parseRunInterval },
  })
  .meta({ description: "Show one stored item" })
  .run(async ({ args, flags }) => {
    const workspacePath = args.item ? args.workspace : ".clankpipe.toml";
    const itemRef = args.item ?? args.workspace;
    const controller = new AbortController();
    const removeHandlers = installCancellationHandlers(controller);
    try {
      const workspace = await loadWorkspace(workspacePath, undefined, controller.signal, false);
      if (flags.watch && flags.json) throw new Error("--watch cannot be combined with --json");
      if (flags.watch) {
        await watchView("show", flags.interval, () => renderShow(workspace, itemRef, false), controller.signal);
      } else {
        process.stdout.write(await renderShow(workspace, itemRef, flags.json === true));
      }
    } finally {
      removeHandlers();
    }
  });

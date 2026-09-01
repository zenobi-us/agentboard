import { app } from "./app.ts";
import { loadWorkspace } from "../services/workspace.ts";
import { readStoreViews } from "../services/store.ts";

function renderDashboard(snapshots: Awaited<ReturnType<typeof readStoreViews>>): string {
  const lines = ["ClankPipe Dashboard", "", "Sources:"];
  for (const snapshot of snapshots) {
    const status = snapshot.collectionStatus;
    const statusText = status ? `  collection=${status.state}` : "";
    const errorText = status?.error ? `  error=${status.error}` : "";
    lines.push(`${snapshot.state === "ready" ? "●" : "○"} ${snapshot.sourceId}  ${snapshot.items.length} items${statusText}${errorText}`);
    for (const value of snapshot.items) {
      lines.push(`  ${value.item.reference_id}\t${value.item.status}\t${value.actionState}\t${value.item.title}`);
    }
  }
  lines.push("", "q or Esc: quit   r: refresh");
  return lines.join("\n");
}

async function openDashboard(workspacePath: string): Promise<void> {
  if (!process.stdin.isTTY || !process.stdout.isTTY) {
    throw new Error("Dashboard requires interactive stdin and stdout");
  }
  const workspace = await loadWorkspace(workspacePath, undefined, undefined, false);
  const refresh = async () => {
    process.stdout.write("\x1b[2J\x1b[H" + renderDashboard(await readStoreViews(workspace)) + "\n");
  };
  await refresh();
  process.stdin.setRawMode?.(true);
  process.stdin.resume();
  try {
    await new Promise<void>((resolve) => {
      const timer = setInterval(refresh, 60_000);
      const onData = (chunk: Buffer) => {
        if (chunk.includes(0x71) || chunk.includes(0x1b)) {
          clearInterval(timer);
          process.stdin.off("data", onData);
          resolve();
        } else if (chunk.includes(0x72)) {
          void refresh();
        }
      };
      process.stdin.on("data", onData);
    });
  } finally {
    process.stdin.setRawMode?.(false);
    process.stdin.pause();
    process.stdout.write("\x1b[0m\n");
  }
}

export const dashboardCmd = app
  .sub("dashboard")
  .args([{ name: "workspace", type: "string", default: ".clankpipe.toml" }])
  .meta({ description: "Open the read-only Store dashboard" })
  .run(async ({ args }) => openDashboard(args.workspace));

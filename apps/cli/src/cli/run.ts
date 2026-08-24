import { appendFile } from "node:fs/promises";
import { loadWorkspace } from "../services/workspace.ts";
import { runWorkspace, watchWorkspace, type WorkspaceRunResult } from "../services/runtime.ts";
import { app } from "./app.ts";
import { installCancellationHandlers } from "./cancellation.ts";

export function runExitStatus(result: WorkspaceRunResult): number {
  if (result.cancelled) return 130;
  return result.sources.some((source) =>
      source.error !== undefined ||
      source.actions.some((action) =>
        action.error !== undefined || action.result?.outcome === "failure"
      )
    )
    ? 1
    : 0;
}

export function parseRunInterval(value: string): number {
  if (!/^\d+(?:\.\d+)?s?$/.test(value)) throw new TypeError("interval must be seconds");
  const milliseconds = Number(value.replace(/s$/, "")) * 1_000;
  if (milliseconds <= 0) throw new TypeError("interval must be greater than zero");
  return milliseconds;
}

type RunFlags = {
  quiet?: boolean;
  verbose?: boolean;
  color?: string;
  "log-file"?: string;
  json?: boolean;
  "output-format"?: string;
};

export async function reportRun(result: WorkspaceRunResult, flags: RunFlags): Promise<void> {
  const event = { ts: new Date().toISOString(), stage: "run.complete", result };
  if (flags["log-file"]) await appendFile(flags["log-file"], `${JSON.stringify(event)}\n`);
  if (flags.json || flags["output-format"] === "json") {
    process.stdout.write(`${JSON.stringify(result)}\n`);
    return;
  }
  if (flags.quiet) return;
  const color = flags.color === "always" ? "\x1b[36m" : "";
  const reset = color ? "\x1b[0m" : "";
  const summary = result.sources.map((source) => `${source.id}: ${source.items.length} items`).join(", ");
  console.error(`${color}run complete${reset}${summary ? `: ${summary}` : ""}`);
  if (flags.verbose) {
    for (const source of result.sources) {
      for (const action of source.actions) {
        const detail = action.result?.message ?? action.error;
        if (detail) console.error(detail);
      }
    }
  }
}

export const runCmd = app
  .sub("run")
  .args([{ name: "workspace", type: "string", default: ".agentboard.toml" }])
  .flags({
    "dry-run": { type: "boolean", description: "Collect and render without Store writes or Action execution" },
    watch: { type: "boolean", description: "Repeat Runs until cancellation" },
    interval: { type: "string", default: "60s", parse: parseRunInterval },
    json: { type: "boolean", description: "Print structured JSON output" },
    "output-format": { type: "string", choices: ["human", "json"], default: "human", aliases: ["format"], description: "Select command output format" },
  })
  .meta({ description: "Execute a Workspace Run" })
  .run(async ({ args, flags }) => {
    const controller = new AbortController();
    const removeHandlers = installCancellationHandlers(controller);
    try {
      const workspace = await loadWorkspace(args.workspace, undefined, controller.signal);
      const result = flags.watch
        ? await watchWorkspace(workspace, {
          intervalMs: flags.interval,
          dryRun: flags["dry-run"],
          onResult: (run) => { void reportRun(run, flags); },
        })
        : await runWorkspace(workspace, { dryRun: flags["dry-run"] });
      if (!flags.watch) await reportRun(result, flags);
      process.exitCode = runExitStatus(result);
    } catch (error) {
      if (controller.signal.aborted) process.exitCode = 130;
      else throw error;
    } finally {
      removeHandlers();
    }
  });

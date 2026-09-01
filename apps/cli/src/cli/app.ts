import { basename } from "node:path";
import { Crust } from "@crustjs/core";

export const CLI_NAME = "clankpipe";
export const LEGACY_CLI_NAME = "agentboard";

export function invokedCliName(path = process.argv[1]): string {
  return basename(path ?? CLI_NAME);
}

export function printLegacyDeprecation(path = process.argv[1]): void {
  if (invokedCliName(path) === LEGACY_CLI_NAME) {
    console.error("agentboard is deprecated; use clankpipe instead.");
  }
}

export const WATCH_STDOUT_ERROR = "Watch Mode requires terminal stdout; do not redirect stdout";

export async function watchView(
  command: string,
  intervalMs: number,
  render: () => Promise<string>,
  cancellation: AbortSignal,
): Promise<void> {
  if (process.stdout.isTTY !== true) throw new Error(WATCH_STDOUT_ERROR);
  let displayed = false;
  do {
    const view = await render();
    if (displayed) process.stdout.write("\x1b[2J\x1b[H");
    process.stdout.write(`${CLI_NAME} ${command} --watch\nInterval: ${intervalMs / 1_000}s\nLast refresh: ${new Date().toISOString()}\n\n${view}`);
    displayed = true;
    if (cancellation.aborted) return;
    await new Promise<void>((resolve) => {
      const onAbort = () => { clearTimeout(timer); resolve(); };
      const timer = setTimeout(() => {
        cancellation.removeEventListener("abort", onAbort);
        resolve();
      }, intervalMs);
      cancellation.addEventListener("abort", onAbort, { once: true });
    });
  } while (!cancellation.aborted);
}

export const app = new Crust(CLI_NAME)
  .flags({
    verbose: { type: "boolean", short: "v", inherit: true, description: "Show detailed progress" },
    quiet: { type: "boolean", short: "q", inherit: true, description: "Suppress non-error progress" },
    color: { type: "string", choices: ["auto", "always", "never"], default: "auto", inherit: true, description: "Control human-readable color" },
    "log-file": { type: "string", inherit: true, description: "Append diagnostic events to a JSONL file" },
  })
  .meta({ description: "Collect task-tracking items into local agent work queues" });

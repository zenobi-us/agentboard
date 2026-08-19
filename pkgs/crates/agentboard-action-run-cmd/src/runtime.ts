import type { ActionResult, ActionRuntime, HealthCheckContext } from "@agentboard/core/config";
import type { RunCmdConfig } from "./config.ts";

const OUTPUT_LIMIT = 64 * 1024;
type ProcessResult = { code: number; stdout: string; stderr: string; cancelled: boolean };

type RunOptions = {
  readonly env?: Record<string, string | undefined>;
};

function stopProcessGroup(child: Bun.Subprocess): void {
  if (process.platform === "win32") {
    // Bun has no portable process-group signal on Windows.
    child.kill();
    return;
  }
  try { process.kill(-child.pid, "SIGTERM"); } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ESRCH") throw error;
  }
}

async function readLimited(stream: ReadableStream<Uint8Array>): Promise<string> {
  const reader = stream.getReader();
  const chunks: Uint8Array[] = [];
  let size = 0;
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      if (size < OUTPUT_LIMIT) {
        const chunk = value.slice(0, OUTPUT_LIMIT - size);
        chunks.push(chunk);
        size += chunk.byteLength;
      }
    }
  } finally {
    reader.releaseLock();
  }
  const output = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) {
    output.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return new TextDecoder().decode(output);
}

function cap(value: string): string {
  const bytes = new TextEncoder().encode(value);
  return new TextDecoder().decode(bytes.slice(0, OUTPUT_LIMIT));
}

function combine(launch: string, probe: string): string {
  if (probe.length === 0) return cap(launch);
  const launchBytes = new TextEncoder().encode(launch);
  const probeBytes = new TextEncoder().encode(probe);
  if (probeBytes.byteLength >= OUTPUT_LIMIT) return new TextDecoder().decode(probeBytes.slice(0, OUTPUT_LIMIT));
  const separator = launchBytes.byteLength > 0 ? 1 : 0;
  const launchLimit = Math.max(0, OUTPUT_LIMIT - probeBytes.byteLength - separator);
  const prefix = new TextDecoder().decode(launchBytes.slice(0, launchLimit));
  return separator === 1 ? `${prefix}\n${probe}` : probe;
}

async function run(
  command: string,
  cwd: string | null | undefined,
  signal: AbortSignal,
  options: RunOptions = {},
): Promise<ProcessResult> {
  if (signal.aborted) throw new Error("action cancelled");
  const child = Bun.spawn(["sh", "-c", command], {
    cwd: cwd ?? undefined,
    env: options.env,
    stdout: "pipe",
    stderr: "pipe",
    detached: true,
  });
  const stop = () => stopProcessGroup(child);
  signal.addEventListener("abort", stop, { once: true });
  try {
    const [code, stdout, stderr] = await Promise.all([
      child.exited,
      readLimited(child.stdout),
      readLimited(child.stderr),
    ]);
    return { code, stdout, stderr, cancelled: signal.aborted };
  } finally { signal.removeEventListener("abort", stop); }
}

function wait(ms: number, signal: AbortSignal): Promise<void> {
  if (signal.aborted) return Promise.resolve();
  return new Promise((resolve) => {
    const done = () => { clearTimeout(timer); signal.removeEventListener("abort", done); resolve(); };
    const timer = setTimeout(done, ms);
    signal.addEventListener("abort", done, { once: true });
    if (signal.aborted) done();
  });
}

export function runtime(_config: RunCmdConfig): ActionRuntime<RunCmdConfig> {
  return {
    execute: async (context): Promise<ActionResult> => {
      const input = context.inputs;
      const env = {
        ...process.env,
        AGENTBOARD_WORKSPACE_ID: context.workspaceId,
        AGENTBOARD_SOURCE_ID: context.sourceId,
        AGENTBOARD_ITEM_ID: context.item.id,
      };
      let stdout = "";
      let stderr = "";
      try {
        const launch = await run(input.cmd, input.cwd, context.cancellation, { env });
        stdout = launch.stdout;
        stderr = launch.stderr;
        if (launch.cancelled) return { outcome: "cancelled", stdout, stderr, message: "action cancelled" };
        if (launch.code !== 0) return { outcome: "failure", stdout, stderr, message: `command exited with ${launch.code}` };
        if (!input.healthcheck) return { outcome: "success", stdout, stderr };
        const deadline = Date.now() + parseDuration(input.healthcheck_timeout ?? "30s");
        while (Date.now() < deadline) {
          const probeController = new AbortController();
          let timedOut = false;
          const timeout = setTimeout(() => { timedOut = true; probeController.abort(); }, Math.max(1, deadline - Date.now()));
          const cancelProbe = () => probeController.abort();
          context.cancellation.addEventListener("abort", cancelProbe, { once: true });
          const probe = await run(input.healthcheck, input.cwd, probeController.signal, { env });
          clearTimeout(timeout);
          context.cancellation.removeEventListener("abort", cancelProbe);
          stdout = combine(stdout, probe.stdout);
          stderr = combine(stderr, probe.stderr);
          if (context.cancellation.aborted) return { outcome: "cancelled", stdout, stderr, message: "action cancelled" };
          if (timedOut) return { outcome: "failure", stdout, stderr, message: `healthcheck timed out after ${input.healthcheck_timeout ?? "30s"}` };
          if (probe.code === 0) return { outcome: "success", stdout, stderr };
          await wait(Math.min(parseDuration(input.healthcheck_interval ?? "1s"), Math.max(0, deadline - Date.now())), context.cancellation);
          if (context.cancellation.aborted) return { outcome: "cancelled", stdout, stderr, message: "action cancelled" };
        }
        return { outcome: "failure", stdout, stderr, message: `healthcheck timed out after ${input.healthcheck_timeout ?? "30s"}` };
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        return { outcome: context.cancellation.aborted ? "cancelled" : "failure", stdout: cap(stdout), stderr: cap(stderr || message), message };
      }
    },
  };
}

export async function healthCheck(config: RunCmdConfig, context: HealthCheckContext): Promise<void> {
  const result = await run(":", config.cwd, context.cancellation);
  if (result.cancelled) throw new Error("action cancelled");
  if (result.code !== 0) throw new Error(`required command sh returned ${result.code}`);
}

export function parseDuration(value: string): number {
  const match = /^(\d+(?:\.\d+)?)(ms|s|m|h)$/.exec(value);
  if (!match) throw new Error(`invalid duration ${value}`);
  const duration = Number(match[1]) * { ms: 1, s: 1000, m: 60_000, h: 3_600_000 }[match[2] as "ms" | "s" | "m" | "h"];
  if (!Number.isFinite(duration) || duration <= 0) throw new Error(`invalid duration ${value}`);
  return duration;
}

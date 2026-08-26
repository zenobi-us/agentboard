import { existsSync, realpathSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import type { ActionResult, ActionRuntime, HealthCheckContext } from "@agentboard/core/config";
import type { LlmConfig } from "./config.ts";

const OUTPUT_LIMIT = 64 * 1024;
type Result = { code: number; stdout: string; stderr: string; cancelled: boolean };

type Command = { argv: string[]; cwd?: string };

function cap(value: string): string {
  return new TextDecoder().decode(new TextEncoder().encode(value).slice(0, OUTPUT_LIMIT));
}

function start(command: Command, env: Record<string, string | undefined>): void {
  Bun.spawn(command.argv, { cwd: command.cwd, env, stdout: "ignore", stderr: "ignore" });
}

async function output(stream: ReadableStream<Uint8Array>): Promise<string> {
  const reader = stream.getReader();
  const chunks: Uint8Array[] = [];
  let size = 0;
  try {
    while (true) {
      const next = await reader.read();
      if (next.done) break;
      if (size < OUTPUT_LIMIT) {
        const chunk = next.value.slice(0, OUTPUT_LIMIT - size);
        chunks.push(chunk);
        size += chunk.byteLength;
      }
    }
  } finally {
    reader.releaseLock();
  }
  const result = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) {
    result.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return new TextDecoder().decode(result);
}

async function run(command: Command, signal: AbortSignal, env: Record<string, string | undefined>): Promise<Result> {
  if (signal.aborted) throw new Error("action cancelled");
  const child = Bun.spawn(command.argv, {
    cwd: command.cwd,
    env,
    stdout: "pipe",
    stderr: "pipe",
    detached: process.platform !== "win32",
  });
  const stop = () => {
    if (process.platform === "win32") child.kill();
    else {
      try { process.kill(-child.pid, "SIGTERM"); } catch (error) {
        if ((error as NodeJS.ErrnoException).code !== "ESRCH") throw error;
      }
    }
  };
  signal.addEventListener("abort", stop, { once: true });
  try {
    const [code, stdout, stderr] = await Promise.all([child.exited, output(child.stdout), output(child.stderr)]);
    return { code, stdout, stderr, cancelled: signal.aborted };
  } finally {
    signal.removeEventListener("abort", stop);
  }
}

async function git(repo: string, args: string[], signal: AbortSignal, env: Record<string, string | undefined>): Promise<Result> {
  return run({ argv: ["git", "-C", repo, ...args] }, signal, env);
}

function canonical(path: string): string {
  const absolute = resolve(path);
  return existsSync(absolute) ? realpathSync(absolute) : absolute;
}

async function ensureWorktree(config: LlmConfig, signal: AbortSignal, env: Record<string, string | undefined>): Promise<string | undefined> {
  if (!config.worktree) return config.cwd ?? undefined;
  const worktree = config.worktree;
  const repo = canonical(worktree.repo);
  const root = canonical(worktree.root);
  const listed = await git(repo, ["worktree", "list", "--porcelain"], signal, env);
  if (listed.code !== 0) throw new Error(listed.stderr || "cannot list Git worktrees");
  const lines = listed.stdout.split("\n");
  const rootLine = `worktree ${root}`;
  const registered = lines.includes(rootLine);
  if (registered) {
    const branchLine = lines[lines.indexOf(rootLine) + 1] ?? "";
    const branch = branchLine.replace(/^branch refs\/heads\//, "");
    if (branch !== worktree.branch) throw new Error(`worktree ${worktree.root} is on branch ${branch}`);
    return root;
  }
  if (existsSync(resolve(worktree.root))) throw new Error(`worktree root ${worktree.root} is not a managed worktree`);
  const branch = await git(repo, ["show-ref", "--verify", "--quiet", `refs/heads/${worktree.branch}`], signal, env);
  const args = branch.code === 0
    ? ["worktree", "add", worktree.root, worktree.branch]
    : ["worktree", "add", "-b", worktree.branch, worktree.root, "HEAD"];
  const created = await git(repo, args, signal, env);
  if (created.code !== 0) throw new Error(created.stderr || `git ${args.join(" ")} failed`);
  return root;
}

export function buildPiArgs(config: LlmConfig, prompt: string): string[] {
  const args: string[] = [config.runner ?? "pi"];
  if (config.provider) args.push("--provider", config.provider);
  if (config.model) args.push("--model", config.model);
  if (config.thinking) args.push("--thinking", config.thinking);
  args.push(prompt);
  return args;
}

function shellQuote(value: string): string {
  return `'${value.replaceAll("'", "'\\''")}'`;
}

type Position = { direction?: "left" | "right" | "up" | "down"; size?: string };

function positionArgs(position: Position | undefined): string[] {
  if (!position?.direction) return [];
  return ["--direction", position.direction, ...(position.size ? ["--size", position.size] : [])];
}

function terminalCommand(terminal: Exclude<NonNullable<LlmConfig["terminal"]>, { kind: "herdr" }>, cwd: string, piArgs: string[], env: Record<string, string | undefined>): Command {
  const name = "name" in terminal ? terminal.name ?? `agentboard-${env["AGENTBOARD_ITEM_ID"] ?? "item"}` : undefined;
  if (terminal.kind === "zellij") {
    if (terminal.container === "session") return { argv: ["zellij", "--session", name!, "--cwd", cwd, "--", ...piArgs] };
    const action = terminal.container === "tab" ? "new-tab" : "new-pane";
    return { argv: ["zellij", "action", action, ...("name" in terminal && name ? ["--name", name] : []), "--cwd", cwd, ...positionArgs(terminal.position), "--", ...piArgs] };
  }
  if (terminal.kind === "tmux") {
    if (terminal.container === "session") return { argv: ["tmux", "new-session", "-d", "-s", name!, "-c", cwd, ...piArgs] };
    const direction = terminal.position?.direction === "left" || terminal.position?.direction === "right" ? "-h" : "-v";
    return { argv: ["tmux", "split-window", direction, "-c", cwd, ...piArgs] };
  }
  if (terminal.kind === "generic") return { argv: [terminal.command, ...(terminal.args ?? []), ...piArgs], cwd };
  throw new Error("unsupported terminal kind");
}

function nameFor(env: Record<string, string | undefined>): string {
  return `agentboard-${env["AGENTBOARD_ITEM_ID"] ?? "item"}`;
}

async function launchHerdr(
  terminal: Extract<NonNullable<LlmConfig["terminal"]>, { kind: "herdr" }>,
  cwd: string,
  piArgs: string[],
  env: Record<string, string | undefined>,
  signal: AbortSignal,
): Promise<Result> {
  const workspace = env["HERDR_WORKSPACE_ID"];
  if (!workspace) throw new Error("HERDR_WORKSPACE_ID is required for the Herdr terminal");
  const opened = terminal.container === "worktree"
    ? await run({ argv: ["herdr", "worktree", "open", "--workspace", workspace, "--path", cwd, "--no-focus"] }, signal, env)
    : terminal.container === "tab"
      ? await run({ argv: ["herdr", "tab", "create", "--workspace", workspace, "--cwd", cwd, "--label", nameFor(env), "--no-focus"] }, signal, env)
      : await run({ argv: ["herdr", "pane", "split", "--current", "--cwd", cwd, "--no-focus", ...positionArgs(terminal.position)] }, signal, env);
  if (opened.code !== 0) return opened;
  const parsed = JSON.parse(opened.stdout) as { result?: { root_pane?: { pane_id?: string }; pane?: { pane_id?: string } } };
  const paneId = parsed.result?.root_pane?.pane_id ?? parsed.result?.pane?.pane_id;
  if (!paneId) throw new Error("Herdr did not return a pane id");
  const command = piArgs.map(shellQuote).join(" ");
  return run({ argv: ["herdr", "pane", "run", paneId, command] }, signal, env);
}

export function runtime(_config: LlmConfig): ActionRuntime<LlmConfig> {
  return {
    execute: async (context): Promise<ActionResult> => {
      const inputs = context.inputs;
      const env = {
        ...process.env,
        AGENTBOARD_WORKSPACE_ID: context.workspaceId,
        AGENTBOARD_SOURCE_ID: context.sourceId,
        AGENTBOARD_ITEM_ID: context.item.id,
      };
      let stdout = "";
      try {
        const prompt = inputs.prompt_file
          ? await readFile(inputs.prompt_file, "utf8")
          : inputs.prompt ?? "";
        const cwd = await ensureWorktree(inputs, context.cancellation, env);
        const piArgs = buildPiArgs(inputs, prompt);
        const command = inputs.terminal && inputs.terminal.kind !== "herdr"
          ? terminalCommand(inputs.terminal, cwd ?? process.cwd(), piArgs, env)
          : { argv: piArgs, cwd };
        if (!inputs.terminal && inputs.mode === "background") {
          start(command, env);
          return { outcome: "success", stdout: "started\n", stderr: "" };
        }
        const result = inputs.terminal?.kind === "herdr"
          ? await launchHerdr(inputs.terminal, cwd ?? process.cwd(), piArgs, env, context.cancellation)
          : await run(command, context.cancellation, env);
        stdout = result.stdout;
        if (result.cancelled) return { outcome: "cancelled", stdout, stderr: result.stderr, message: "action cancelled" };
        if (result.code !== 0) return { outcome: "failure", stdout, stderr: result.stderr, message: `command exited with ${result.code}` };
        return { outcome: "success", stdout, stderr: result.stderr };
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        return { outcome: context.cancellation.aborted ? "cancelled" : "failure", stdout: cap(stdout), stderr: message, message };
      }
    },
  };
}

export async function healthCheck(config: LlmConfig, context: HealthCheckContext): Promise<void> {
  const command = config.terminal?.kind === "herdr" ? "herdr" : config.terminal?.kind === "zellij" ? "zellij" : config.terminal?.kind === "tmux" ? "tmux" : config.terminal?.kind === "generic" ? config.terminal.command : config.runner ?? "pi";
  const result = await run({ argv: [command, "--version"] }, context.cancellation, process.env);
  if (result.cancelled) throw new Error("action cancelled");
  if (result.code !== 0) throw new Error(result.stderr || `required command ${command} returned ${result.code}`);
}

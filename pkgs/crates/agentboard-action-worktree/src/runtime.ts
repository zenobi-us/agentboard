import { existsSync, realpathSync } from "node:fs";
import { resolve } from "node:path";
import type { ActionResult, ActionRuntime, HealthCheckContext } from "@agentboard/core/config";
import type { WorktreeConfig } from "./config.ts";

type GitResult = { code: number; stdout: string; stderr: string };
type RegisteredWorktree = { path: string; branch?: string };

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

async function git(path: string, args: string[], signal: AbortSignal): Promise<GitResult> {
  if (signal.aborted) throw new Error("worktree action cancelled");
  const child = Bun.spawn(["git", "-C", path, ...args], { stdout: "pipe", stderr: "pipe", detached: true });
  const stop = () => stopProcessGroup(child);
  signal.addEventListener("abort", stop, { once: true });
  try {
    const code = await child.exited;
    const [stdout, stderr] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text()]);
    if (signal.aborted) throw new Error("worktree action cancelled");
    return { code, stdout: stdout.trim(), stderr: stderr.trim() };
  } finally { signal.removeEventListener("abort", stop); }
}

function canonical(path: string): string {
  const absolute = resolve(path);
  return existsSync(absolute) ? realpathSync(absolute) : absolute;
}

async function repositoryIdentity(path: string, signal: AbortSignal): Promise<string> {
  const result = await git(path, ["rev-parse", "--git-common-dir"], signal);
  if (result.code !== 0) throw new Error(result.stderr || `${path} is not a Git repository`);
  return canonical(resolve(path, result.stdout));
}

async function registeredWorktrees(repo: string, signal: AbortSignal): Promise<RegisteredWorktree[]> {
  const result = await git(repo, ["worktree", "list", "--porcelain"], signal);
  if (result.code !== 0) throw new Error(result.stderr || "cannot list Git worktrees");
  const entries: RegisteredWorktree[] = [];
  let current: RegisteredWorktree | undefined;
  for (const line of result.stdout.split("\n")) {
    if (line.startsWith("worktree ")) {
      current = { path: canonical(line.slice("worktree ".length)) };
      entries.push(current);
    } else if (line.startsWith("branch ") && current) {
      current.branch = line.slice("branch ".length).replace(/^refs\/heads\//, "");
    }
  }
  return entries;
}

async function branchExists(repo: string, branch: string, signal: AbortSignal): Promise<boolean> {
  const result = await git(repo, ["show-ref", "--verify", "--quiet", `refs/heads/${branch}`], signal);
  return result.code === 0;
}

async function ensure(config: WorktreeConfig, signal: AbortSignal): Promise<string> {
  const repo = canonical(config.repo);
  const root = canonical(config.root);
  const repoIdentity = await repositoryIdentity(repo, signal);
  const worktrees = await registeredWorktrees(repo, signal);
  const managed = worktrees.find((entry) => entry.path === root);

  if (managed && await repositoryIdentity(root, signal) !== repoIdentity) {
    throw new Error(`worktree root ${config.root} belongs to another repository`);
  }
  if (managed?.branch === config.branch) return `reused ${config.root}\n`;
  if (worktrees.some((entry) => entry.path !== root && entry.branch === config.branch)) {
    throw new Error(`branch ${config.branch} is already checked out in another worktree`);
  }

  if (managed) {
    const status = await git(root, ["status", "--porcelain", "--untracked-files=all"], signal);
    if (status.code !== 0) throw new Error(status.stderr || `cannot inspect worktree ${config.root}`);
    if (status.stdout) throw new Error(`managed worktree ${config.root} is dirty`);
    const result = await git(
      root,
      await branchExists(repo, config.branch, signal)
        ? ["switch", config.branch]
        : ["switch", "-c", config.branch, await git(repo, ["rev-parse", "HEAD"], signal).then((head) => head.stdout)],
      signal,
    );
    if (result.code !== 0) throw new Error(result.stderr || `cannot switch worktree ${config.root}`);
    return result.stdout;
  }

  if (existsSync(resolve(config.root))) {
    throw new Error(`worktree root ${config.root} is not a managed worktree`);
  }
  const args = await branchExists(repo, config.branch, signal)
    ? ["worktree", "add", config.root, config.branch]
    : ["worktree", "add", "-b", config.branch, config.root, "HEAD"];
  const result = await git(repo, args, signal);
  if (result.code !== 0) throw new Error(result.stderr || `git ${args.join(" ")} failed`);
  return result.stdout;
}

export function runtime(_config: WorktreeConfig): ActionRuntime<WorktreeConfig> {
  return {
    cachedSuccessIsValid: async ({ cancellation, inputs }) => {
      try {
        const repoIdentity = await repositoryIdentity(inputs.repo, cancellation);
        const rootIdentity = await repositoryIdentity(inputs.root, cancellation);
        if (repoIdentity !== rootIdentity) return false;
        const managed = (await registeredWorktrees(inputs.repo, cancellation))
          .some((worktree) => worktree.path === canonical(inputs.root));
        if (!managed) return false;
        const result = await git(inputs.root, ["branch", "--show-current"], cancellation);
        return result.code === 0 && result.stdout === inputs.branch;
      } catch { return false; }
    },
    execute: async ({ cancellation, inputs }): Promise<ActionResult> => {
      try {
        return { outcome: "success", stdout: await ensure(inputs, cancellation), stderr: "" };
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        return { outcome: cancellation.aborted ? "cancelled" : "failure", stdout: "", stderr: message, message };
      }
    },
  };
}

export async function healthCheck(config: WorktreeConfig, context: HealthCheckContext): Promise<void> {
  const result = await git(config.repo, ["--version"], context.cancellation);
  if (result.code !== 0) throw new Error(`required command git returned ${result.code}`);
}

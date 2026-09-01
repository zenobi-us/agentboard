import { expect, test } from "bun:test";
import { mkdir, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { runtime } from "./runtime.ts";

async function git(cwd: string, ...args: string[]): Promise<void> {
  const result = Bun.spawnSync(["git", "-C", cwd, ...args], { stdout: "pipe", stderr: "pipe" });
  if (result.exitCode !== 0) throw new Error(new TextDecoder().decode(result.stderr));
}

function inputs(repo: string, root: string, branch: string) {
  return { repo, root, branch };
}

test("creates, reuses, and switches a clean managed worktree", async () => {
  const directory = await mkdtemp(join(tmpdir(), "agentboard-worktree-"));
  const repo = join(directory, "repo");
  const root = join(directory, "worktree");
  await mkdir(repo);
  await git(repo, "init", "-q", "-b", "main");
  await Bun.write(join(repo, "file"), "content");
  await git(repo, "add", "file");
  await git(repo, "-c", "user.name=AgentBoard", "-c", "user.email=agentboard@example.test", "commit", "-qm", "initial");

  const action = runtime(inputs(repo, root, "agentboard/item"));
  await expect(action.execute({ workspaceId: "w", sourceId: "s", item: {} as never, inputs: inputs(repo, root, "agentboard/item"), cancellation: new AbortController().signal })).resolves.toMatchObject({ outcome: "success" });
  await expect(action.execute({ workspaceId: "w", sourceId: "s", item: {} as never, inputs: inputs(repo, root, "agentboard/item"), cancellation: new AbortController().signal })).resolves.toMatchObject({ outcome: "success" });

  await git(repo, "branch", "other");
  const switchResult = await action.execute({ workspaceId: "w", sourceId: "s", item: {} as never, inputs: inputs(repo, root, "other"), cancellation: new AbortController().signal });
  expect(switchResult.outcome).toBe("success");
  await rm(directory, { recursive: true, force: true });
});

test("creates a missing branch from the repository HEAD in a managed worktree", async () => {
  const directory = await mkdtemp(join(tmpdir(), "agentboard-worktree-"));
  const repo = join(directory, "repo");
  const root = join(directory, "worktree");
  await mkdir(repo);
  await git(repo, "init", "-q", "-b", "main");
  await Bun.write(join(repo, "file"), "content");
  await git(repo, "add", "file");
  await git(repo, "-c", "user.name=AgentBoard", "-c", "user.email=agentboard@example.test", "commit", "-qm", "initial");

  const first = inputs(repo, root, "first");
  await expect(runtime(first).execute({ workspaceId: "w", sourceId: "s", item: {} as never, inputs: first, cancellation: new AbortController().signal })).resolves.toMatchObject({ outcome: "success" });
  await git(repo, "switch", "-c", "main-only");
  await Bun.write(join(repo, "repo-only"), "repo");
  await git(repo, "add", "repo-only");
  await git(repo, "-c", "user.name=AgentBoard", "-c", "user.email=agentboard@example.test", "commit", "-qm", "repo head");

  const second = inputs(repo, root, "second");
  const result = await runtime(second).execute({ workspaceId: "w", sourceId: "s", item: {} as never, inputs: second, cancellation: new AbortController().signal });
  expect(result.outcome).toBe("success");
  expect(await Bun.file(join(root, "repo-only")).exists()).toBe(true);
  await rm(directory, { recursive: true, force: true });
});

test("does not accept an unregistered directory as a cached worktree", async () => {
  const directory = await mkdtemp(join(tmpdir(), "agentboard-worktree-"));
  const repo = join(directory, "repo");
  const root = join(directory, "nested");
  await mkdir(repo);
  await git(repo, "init", "-q", "-b", "main");
  await Bun.write(join(repo, "file"), "content");
  await git(repo, "add", "file");
  await git(repo, "-c", "user.name=AgentBoard", "-c", "user.email=agentboard@example.test", "commit", "-qm", "initial");
  await mkdir(root);

  const config = inputs(repo, root, "main");
  const valid = await runtime(config).cachedSuccessIsValid!({
    workspaceId: "w",
    sourceId: "s",
    item: {} as never,
    inputs: config,
    cancellation: new AbortController().signal,
  });
  expect(valid).toBe(false);
  await rm(directory, { recursive: true, force: true });
});

test("rejects the repository root, an unrelated existing root, and branch collision", async () => {
  const directory = await mkdtemp(join(tmpdir(), "agentboard-worktree-"));
  const repo = join(directory, "repo");
  const other = join(directory, "other");
  const root = join(directory, "root");
  await mkdir(repo);
  await mkdir(other);
  await git(repo, "init", "-q", "-b", "main");
  await Bun.write(join(repo, "file"), "content");
  await git(repo, "add", "file");
  await git(repo, "-c", "user.name=AgentBoard", "-c", "user.email=agentboard@example.test", "commit", "-qm", "initial");
  await Bun.write(join(other, "file"), "unrelated");

  const action = runtime(inputs(repo, root, "agentboard/item"));
  const repositoryRoot = await action.execute({ workspaceId: "w", sourceId: "s", item: {} as never, inputs: inputs(repo, repo, "agentboard/item"), cancellation: new AbortController().signal });
  expect(repositoryRoot.outcome).toBe("failure");

  const unrelated = await action.execute({ workspaceId: "w", sourceId: "s", item: {} as never, inputs: inputs(repo, other, "agentboard/item"), cancellation: new AbortController().signal });
  expect(unrelated.outcome).toBe("failure");

  await git(repo, "branch", "agentboard/collision");
  const collisionRoot = join(directory, "collision");
  await git(repo, "worktree", "add", "-q", collisionRoot, "agentboard/collision");
  const collision = await action.execute({ workspaceId: "w", sourceId: "s", item: {} as never, inputs: inputs(repo, root, "agentboard/collision"), cancellation: new AbortController().signal });
  expect(collision.outcome).toBe("failure");
  await rm(directory, { recursive: true, force: true });
});

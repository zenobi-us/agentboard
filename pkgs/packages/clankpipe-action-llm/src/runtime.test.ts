import { expect, test } from "bun:test";
import { chmod, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import plugin from "./config.ts";
import { buildHarnessArgs, resolveHerdrCwd, runtime } from "./runtime.ts";

test("requires exactly one prompt input", () => {
  expect(() => plugin.validate!({})).toThrow("exactly one of prompt or prompt_file is required");
  expect(() => plugin.validate!({ prompt: "one", prompt_file: "two" })).toThrow("exactly one of prompt or prompt_file is required");
  expect(() => plugin.validate!({ prompt: "one" })).not.toThrow();
});

test("builds harness arguments with configured options", () => {
  expect(buildHarnessArgs({ kind: "herdr", container: "tab", harness: "pi", harness_args: ["--approve"] }, "fix 'quoted'"))
    .toEqual(["pi", "--approve", "fix 'quoted'"]);
});

test("preserves explicit session controls", () => {
  expect(buildHarnessArgs({ kind: "herdr", container: "tab", harness: "pi", harness_args: ["--no-session"] }, "fix 'quoted'"))
    .toEqual(["pi", "--no-session", "fix 'quoted'"]);
  expect(buildHarnessArgs({ kind: "herdr", container: "tab", harness: "pi", harness_args: ["--session-id", "existing"] }, "fix 'quoted'"))
    .toEqual(["pi", "--session-id", "existing", "fix 'quoted'"]);
});

test("passes ClankPipe and AgentBoard environment names to the harness", async () => {
  const result = await runtime({ prompt: "ignored" } as never).execute({
    workspaceId: "workspace",
    sourceId: "source",
    item: { id: "item" } as never,
    inputs: {
      prompt: "ignored",
      terminal: {
        kind: "generic",
        command: "sh",
        args: ["-c", "printf '%s|%s' \"$CLANKPIPE_ITEM_ID\" \"$AGENTBOARD_ITEM_ID\""],
      },
    } as never,
    cancellation: new AbortController().signal,
  });

  expect(result).toMatchObject({ outcome: "success", stdout: "item|item" });
});

test("resolves relative Herdr working directories", () => {
  expect(resolveHerdrCwd("../AgentBoard.worktrees/issue-63")).toBe(
    resolve("../AgentBoard.worktrees/issue-63"),
  );
});

test("keeps a direct background agent running until the child exits", async () => {
  const root = await mkdtemp(join(tmpdir(), "agentboard-llm-"));
  const pi = join(root, "pi");
  await writeFile(pi, "#!/bin/sh\nsleep 0.05\nprintf done\n");
  await chmod(pi, 0o755);
  const previousPath = process.env["PATH"];
  process.env["PATH"] = `${root}:${previousPath ?? ""}`;
  try {
    const result = await runtime({}).execute({
      workspaceId: "workspace",
      sourceId: "source",
      item: { id: "item" } as never,
      inputs: { prompt: "finish", mode: "background" } as never,
      cancellation: new AbortController().signal,
    });
    expect(result).toMatchObject({ outcome: "running", message: "launch accepted; completion pending" });
    expect(result.outcome === "running" ? await result.completion : undefined).toMatchObject({
      outcome: "success",
      stdout: "done",
    });
  } finally {
    if (previousPath === undefined) delete process.env["PATH"];
    else process.env["PATH"] = previousPath;
    await rm(root, { recursive: true, force: true });
  }
});

test("rejects a cwd that differs from the worktree root", () => {
  expect(() => plugin.validate!({
    prompt: "one",
    terminal: { kind: "herdr", container: "tab", cwd: "/repo" },
    worktree: { repo: "/repo", root: "/other", branch: "agentboard/item" },
  })).toThrow("terminal.cwd must match worktree.root");
});

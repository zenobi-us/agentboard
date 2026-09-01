import { expect, test } from "bun:test";
import { resolve } from "node:path";
import plugin from "./config.ts";
import { buildHarnessArgs, resolveHerdrCwd } from "./runtime.ts";

test("requires exactly one prompt input", () => {
  expect(() => plugin.validate!({})).toThrow("exactly one of prompt or prompt_file is required");
  expect(() => plugin.validate!({ prompt: "one", prompt_file: "two" })).toThrow("exactly one of prompt or prompt_file is required");
  expect(() => plugin.validate!({ prompt: "one" })).not.toThrow();
});

test("builds harness arguments with configured options", () => {
  expect(buildHarnessArgs({ kind: "herdr", container: "tab", harness: "pi", harness_args: ["--no-session"] }, "fix 'quoted'"))
    .toEqual(["pi", "--no-session", "fix 'quoted'"]);
});

test("defaults to Pi for direct launches", () => {
  expect(buildHarnessArgs(undefined, "fix 'quoted'"))
    .toEqual(["pi", "fix 'quoted'"]);
});

test("resolves relative Herdr working directories", () => {
  expect(resolveHerdrCwd("../AgentBoard.worktrees/issue-63")).toBe(
    resolve("../AgentBoard.worktrees/issue-63"),
  );
});

test("rejects a cwd that differs from the worktree root", () => {
  expect(() => plugin.validate!({
    prompt: "one",
    terminal: { kind: "herdr", container: "tab", cwd: "/repo" },
    worktree: { repo: "/repo", root: "/other", branch: "agentboard/item" },
  })).toThrow("terminal.cwd must match worktree.root");
});

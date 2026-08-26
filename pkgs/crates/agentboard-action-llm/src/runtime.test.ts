import { expect, test } from "bun:test";
import plugin from "./config.ts";
import { buildPiArgs } from "./runtime.ts";

test("requires exactly one prompt input", () => {
  expect(() => plugin.validate!({})).toThrow("exactly one of prompt or prompt_file is required");
  expect(() => plugin.validate!({ prompt: "one", prompt_file: "two" })).toThrow("exactly one of prompt or prompt_file is required");
  expect(() => plugin.validate!({ prompt: "one" })).not.toThrow();
});

test("builds Pi arguments without shell interpolation", () => {
  expect(buildPiArgs({ prompt: "ignored", provider: "openai", model: "gpt-5", thinking: "high" }, "fix 'quoted'"))
    .toEqual(["pi", "--provider", "openai", "--model", "gpt-5", "--thinking", "high", "fix 'quoted'"]);
});

test("rejects a cwd that differs from the worktree root", () => {
  expect(() => plugin.validate!({
    prompt: "one",
    cwd: "/repo",
    worktree: { repo: "/repo", root: "/other", branch: "agentboard/item" },
  })).toThrow("cwd must match worktree.root");
});

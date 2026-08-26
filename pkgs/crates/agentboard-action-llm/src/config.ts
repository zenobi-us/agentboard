import Type from "typebox";

import { definePlugin } from "@agentboard/core/config";
import { healthCheck, runtime } from "./runtime.ts";

const PositionSchema = Type.Object({
  direction: Type.Optional(Type.Union([
    Type.Literal("left"), Type.Literal("right"), Type.Literal("up"), Type.Literal("down"),
  ])),
  size: Type.Optional(Type.String()),
});

const WorktreeSchema = Type.Object({
  repo: Type.String(),
  root: Type.String(),
  branch: Type.String(),
});

const ZellijTerminalSchema = Type.Object({
  kind: Type.Literal("zellij"),
  container: Type.Union([Type.Literal("session"), Type.Literal("tab"), Type.Literal("pane")]),
  name: Type.Optional(Type.String()),
  position: Type.Optional(PositionSchema),
});

const HerdrTerminalSchema = Type.Object({
  kind: Type.Literal("herdr"),
  container: Type.Union([Type.Literal("worktree"), Type.Literal("tab"), Type.Literal("pane")]),
  position: Type.Optional(PositionSchema),
});

const TmuxTerminalSchema = Type.Object({
  kind: Type.Literal("tmux"),
  container: Type.Union([Type.Literal("session"), Type.Literal("pane")]),
  name: Type.Optional(Type.String()),
  position: Type.Optional(PositionSchema),
});

const GenericTerminalSchema = Type.Object({
  kind: Type.Literal("generic"),
  command: Type.String(),
  args: Type.Optional(Type.Array(Type.String())),
});

export const LlmConfigSchema = Type.Object({
  prompt: Type.Optional(Type.String()),
  prompt_file: Type.Optional(Type.String()),
  runner: Type.Optional(Type.Literal("pi", { default: "pi" })),
  provider: Type.Optional(Type.String()),
  model: Type.Optional(Type.String()),
  thinking: Type.Optional(Type.Union([
    Type.Literal("off"), Type.Literal("minimal"), Type.Literal("low"),
    Type.Literal("medium"), Type.Literal("high"), Type.Literal("xhigh"), Type.Literal("max"),
  ])),
  cwd: Type.Optional(Type.Union([Type.String(), Type.Null()])),
  mode: Type.Optional(Type.Union([Type.Literal("foreground"), Type.Literal("background")], { default: "foreground" })),
  worktree: Type.Optional(WorktreeSchema),
  terminal: Type.Optional(Type.Union([
    ZellijTerminalSchema,
    HerdrTerminalSchema,
    TmuxTerminalSchema,
    GenericTerminalSchema,
  ])),
});

export type LlmConfig = Type.Static<typeof LlmConfigSchema>;

export const LlmActionSchema = Type.Object({
  id: Type.Optional(Type.String({ pattern: "^[A-Za-z_][A-Za-z0-9_]*$" })),
  uses: Type.Literal("@agentboard/action-llm"),
  with: LlmConfigSchema,
});

export type LlmAction = Type.Static<typeof LlmActionSchema>;

export default definePlugin(import.meta, {
  kind: "action",
  schema: LlmConfigSchema,
  validate: (config) => {
    if ((config.prompt === undefined) === (config.prompt_file === undefined)) {
      throw new Error("exactly one of prompt or prompt_file is required");
    }
    if (config.worktree && config.cwd && config.cwd !== config.worktree.root) {
      throw new Error("cwd must match worktree.root when both are set");
    }
  },
  pathInputs: ["cwd", "prompt_file", "worktree.repo", "worktree.root"],
  runtime: (config) => runtime(config),
  healthCheck,
});

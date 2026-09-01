import Type from "typebox";

import { definePlugin } from "@clankpipe/core/config";
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

const TerminalFields = {
  harness: Type.Optional(Type.String({ default: "pi" })),
  harness_args: Type.Optional(Type.Array(Type.String())),
  cwd: Type.Optional(Type.Union([Type.String(), Type.Null()])),
};

const ZellijTerminalSchema = Type.Object({
  ...TerminalFields,
  kind: Type.Literal("zellij"),
  container: Type.Union([Type.Literal("session"), Type.Literal("tab"), Type.Literal("pane")]),
  name: Type.Optional(Type.String()),
  position: Type.Optional(PositionSchema),
});

const HerdrTerminalSchema = Type.Object({
  ...TerminalFields,
  kind: Type.Literal("herdr"),
  container: Type.Union([Type.Literal("worktree"), Type.Literal("tab"), Type.Literal("pane")]),
  position: Type.Optional(PositionSchema),
});

const TmuxTerminalSchema = Type.Object({
  ...TerminalFields,
  kind: Type.Literal("tmux"),
  container: Type.Union([Type.Literal("session"), Type.Literal("pane")]),
  name: Type.Optional(Type.String()),
  position: Type.Optional(PositionSchema),
});

const GenericTerminalSchema = Type.Object({
  ...TerminalFields,
  kind: Type.Literal("generic"),
  command: Type.String(),
  args: Type.Optional(Type.Array(Type.String())),
});

export const LlmConfigSchema = Type.Object({
  prompt: Type.Optional(Type.String()),
  prompt_file: Type.Optional(Type.String()),
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
  uses: Type.Literal("@clankpipe/action-llm"),
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
    const cwd = config.terminal?.cwd;
    if (config.worktree && cwd && cwd !== config.worktree.root) {
      throw new Error("terminal.cwd must match worktree.root when both are set");
    }
  },
  pathInputs: ["prompt_file", "terminal.cwd", "worktree.repo", "worktree.root"],
  runtime: (config) => runtime(config),
  healthCheck,
});

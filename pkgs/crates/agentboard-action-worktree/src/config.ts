import Type from "typebox";

import { definePlugin } from "@agentboard/core/config";
import { healthCheck, runtime } from "./runtime.ts";

export const WorktreeConfigSchema = Type.Object({
  repo: Type.String(),
  root: Type.String(),
  branch: Type.String(),
});

export type WorktreeConfig = Type.Static<typeof WorktreeConfigSchema>;

export const WorktreeActionSchema = Type.Object({
  id: Type.Optional(
    Type.String({ pattern: "^[A-Za-z_][A-Za-z0-9_]*$" }),
  ),
  uses: Type.Literal("@agentboard/action-worktree"),
  with: WorktreeConfigSchema,
});

export type WorktreeAction = Type.Static<typeof WorktreeActionSchema>;

export default definePlugin(import.meta, {
  kind: "action",
  schema: WorktreeConfigSchema,
  validate: () => undefined,
  pathInputs: ["repo", "root"],
  runtime: (config) => runtime(config),
  healthCheck,
});

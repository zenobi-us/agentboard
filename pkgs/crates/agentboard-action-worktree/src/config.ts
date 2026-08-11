import Type from "typebox";

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
  uses: Type.Literal("agentboard/worktree"),
  with: WorktreeConfigSchema,
});

export type WorktreeAction = Type.Static<typeof WorktreeActionSchema>;

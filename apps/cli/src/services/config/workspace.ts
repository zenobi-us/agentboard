import Type from "typebox";

import { RunCmdActionSchema } from "@agentboard/action-run-cmd";
import { WorktreeActionSchema } from "@agentboard/action-worktree";
import { GithubSourceSchema } from "@agentboard/source-github";
import { JiraSourceSchema } from "@agentboard/source-jira";
import { QmdSourceSchema } from "@agentboard/source-qmd";

export const SourceConfigSchema = Type.Union([
  GithubSourceSchema,
  JiraSourceSchema,
  QmdSourceSchema,
]);

export type SourceConfig = Type.Static<typeof SourceConfigSchema>;

export const ActionConfigSchema = Type.Union([
  RunCmdActionSchema,
  WorktreeActionSchema,
]);

export type ActionConfig = Type.Static<typeof ActionConfigSchema>;

export const WorkspaceSourceSchema = Type.Object({
  id: Type.String(),
  source: SourceConfigSchema,
  actions: Type.Optional(Type.Array(ActionConfigSchema, { default: [] })),
});

export type WorkspaceSource = Type.Static<typeof WorkspaceSourceSchema>;

export const WorkspaceConfigSchema = Type.Object({
  sources: Type.Array(WorkspaceSourceSchema),
});

export type WorkspaceConfig = Type.Static<typeof WorkspaceConfigSchema>;

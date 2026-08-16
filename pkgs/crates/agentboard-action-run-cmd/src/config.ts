import Type from "typebox";

import { definePlugin } from "@agentboard/core/config";

export const RunCmdConfigSchema = Type.Object({
  cmd: Type.String(),
  cwd: Type.Optional(Type.Union([Type.String(), Type.Null()])),
  healthcheck: Type.Optional(Type.Union([Type.String(), Type.Null()])),
  healthcheck_interval: Type.Optional(Type.String({ default: "1s" })),
  healthcheck_timeout: Type.Optional(Type.String({ default: "30s" })),
});

export type RunCmdConfig = Type.Static<typeof RunCmdConfigSchema>;

export const RunCmdActionSchema = Type.Object({
  id: Type.Optional(
    Type.String({ pattern: "^[A-Za-z_][A-Za-z0-9_]*$" }),
  ),
  uses: Type.Literal("@agentboard/action-run-cmd"),
  with: RunCmdConfigSchema,
});

export type RunCmdAction = Type.Static<typeof RunCmdActionSchema>;

export default definePlugin(import.meta, {
  kind: "action",
  schema: RunCmdConfigSchema,
  validate: () => undefined,
  pathInputs: ["cwd"],
  runtime: () => ({
    execute: () => ({
      outcome: "failure" as const,
      stdout: "",
      stderr: "Run Command Bun Action runtime is not available",
    }),
  }),
  healthCheck: () => Promise.reject(new Error("Run Command Bun Action runtime is not available")),
});

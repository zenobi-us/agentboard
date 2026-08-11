import Type from "typebox";

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
  uses: Type.Literal("agentboard/run-cmd"),
  with: RunCmdConfigSchema,
});

export type RunCmdAction = Type.Static<typeof RunCmdActionSchema>;

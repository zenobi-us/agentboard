import Type from "typebox";

import { FieldMapSchema } from "@agentboard/core/config";

export const JiraCredentialSchema = Type.Object({
  helper: Type.String(),
});

export type JiraCredential = Type.Static<typeof JiraCredentialSchema>;

export const JiraSourceSchema = Type.Object({
  kind: Type.Literal("jira"),
  site: Type.String(),
  email_env: Type.Optional(Type.String({ default: "JIRA_EMAIL" })),
  token_env: Type.Optional(Type.String({ default: "JIRA_API_TOKEN" })),
  credentials: Type.Optional(
    Type.Union([JiraCredentialSchema, Type.Null()], { default: null }),
  ),
  jql: Type.String(),
  limit: Type.Optional(Type.Integer({ default: 50, minimum: 0 })),
  fields: Type.Optional(Type.Array(Type.String(), { default: [] })),
  field_map: Type.Optional(FieldMapSchema),
  status_map: Type.Optional(
    Type.Record(Type.String(), Type.String(), { default: {} }),
  ),
});

export type JiraSource = Type.Static<typeof JiraSourceSchema>;

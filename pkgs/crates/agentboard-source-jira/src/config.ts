import Type from "typebox";

import { definePlugin, FieldMapSchema } from "@agentboard/core/config";
import { healthCheck, runtime } from "./runtime.ts";

export const JiraCredentialSchema = Type.Object({
  helper: Type.String({ minLength: 1, pattern: "\\S" }),
});

export type JiraCredential = Type.Static<typeof JiraCredentialSchema>;

export const JiraSourceSchema = Type.Object({
  site: Type.String({ minLength: 1, pattern: "^https?://\\S+$" }),
  email_env: Type.Optional(Type.String({ default: "JIRA_EMAIL", minLength: 1, pattern: "\\S" })),
  token_env: Type.Optional(Type.String({ default: "JIRA_API_TOKEN", minLength: 1, pattern: "\\S" })),
  credentials: Type.Optional(
    Type.Union([JiraCredentialSchema, Type.Null()], { default: null }),
  ),
  jql: Type.String({ minLength: 1, pattern: "\\S" }),
  limit: Type.Optional(Type.Integer({ default: 50, minimum: 1 })),
  fields: Type.Optional(Type.Array(Type.String({ minLength: 1, pattern: "\\S" }), { default: [] })),
  field_map: Type.Optional(FieldMapSchema),
  status_map: Type.Optional(
    Type.Record(Type.String(), Type.String(), { default: {} }),
  ),
});

export type JiraSource = Type.Static<typeof JiraSourceSchema>;

export default definePlugin(import.meta, {
  kind: "source",
  schema: JiraSourceSchema,
  itemBucketIdentity: (config) => {
    try {
      const site = new URL(config.site);
      return `${site.host}${site.pathname.replace(/\/+$/, "")}`.toLowerCase();
    } catch {
      return config.site.toLowerCase().replace(/^https?:\/\//, "").replace(/\/$/, "");
    }
  },
  runtime,
  healthCheck,
});

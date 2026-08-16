import Type from "typebox";

import { definePlugin, FieldMapSchema } from "@agentboard/core/config";

export const JiraCredentialSchema = Type.Object({
  helper: Type.String(),
});

export type JiraCredential = Type.Static<typeof JiraCredentialSchema>;

export const JiraSourceSchema = Type.Object({
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
  runtime: () => ({
    collect: () => Promise.reject(new Error("Jira Bun Source runtime is not available")),
  }),
  healthCheck: () => Promise.reject(new Error("Jira Bun Source runtime is not available")),
});

import Type from "typebox";

import { definePlugin, FieldMapSchema } from "@agentboard/core/config";

export const GithubCredentialSchema = Type.Object({
  helper: Type.String(),
});

export type GithubCredential = Type.Static<typeof GithubCredentialSchema>;

export const GithubSourceSchema = Type.Object({
  mode: Type.Literal("issue"),
  query: Type.String(),
  credentials: GithubCredentialSchema,
  limit: Type.Optional(Type.Integer({ default: 50, minimum: 0 })),
  field_map: Type.Optional(FieldMapSchema),
  status_map: Type.Record(Type.String(), Type.String()),
});

export type GithubSource = Type.Static<typeof GithubSourceSchema>;

export default definePlugin(import.meta, {
  kind: "source",
  schema: GithubSourceSchema,
  itemBucketIdentity: () => "github.com",
  runtime: () => ({
    collect: () => Promise.reject(new Error("GitHub Bun Source runtime is not available")),
  }),
  healthCheck: () => Promise.reject(new Error("GitHub Bun Source runtime is not available")),
});

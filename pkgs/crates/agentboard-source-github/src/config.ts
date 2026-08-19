import Type from "typebox";

import { definePlugin, FieldMapSchema } from "@agentboard/core/config";
import { healthCheck, runtime } from "./runtime.ts";

export const GithubCredentialSchema = Type.Object({
  helper: Type.String({ minLength: 1, pattern: "\\S" }),
});

export type GithubCredential = Type.Static<typeof GithubCredentialSchema>;

export const GithubSourceSchema = Type.Object({
  mode: Type.Literal("issue"),
  query: Type.String({ minLength: 1, pattern: "\\S" }),
  credentials: GithubCredentialSchema,
  limit: Type.Optional(Type.Integer({ default: 50, minimum: 1 })),
  field_map: Type.Optional(FieldMapSchema),
  status_map: Type.Record(Type.String({ minLength: 1, pattern: "\\S" }), Type.String({ minLength: 1, pattern: "\\S" }), { minProperties: 1 }),
});

export type GithubSource = Type.Static<typeof GithubSourceSchema>;

export default definePlugin(import.meta, {
  kind: "source",
  schema: GithubSourceSchema,
  itemBucketIdentity: () => "github.com",
  runtime,
  healthCheck,
});

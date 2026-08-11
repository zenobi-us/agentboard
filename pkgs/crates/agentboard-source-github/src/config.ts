import Type from "typebox";

import { FieldMapSchema } from "@agentboard/core/config";

export const GithubCredentialSchema = Type.Object({
  helper: Type.String(),
});

export type GithubCredential = Type.Static<typeof GithubCredentialSchema>;

export const GithubSourceSchema = Type.Object({
  kind: Type.Literal("github"),
  mode: Type.Literal("issue"),
  query: Type.String(),
  credentials: GithubCredentialSchema,
  limit: Type.Optional(Type.Integer({ default: 50, minimum: 0 })),
  field_map: Type.Optional(FieldMapSchema),
  status_map: Type.Record(Type.String(), Type.String()),
});

export type GithubSource = Type.Static<typeof GithubSourceSchema>;

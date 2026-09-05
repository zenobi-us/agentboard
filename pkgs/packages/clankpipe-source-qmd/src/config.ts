import Type from "typebox";

import { definePlugin, FieldMapSchema } from "@clankpipe/core/config";
import { healthCheck, runtime } from "./runtime.ts";

export const QmdSourceSchema = Type.Object({
  collections: Type.Array(Type.String({ minLength: 1, pattern: "\\S" }), { minItems: 1 }),
  query: Type.String({ minLength: 1, pattern: "\\S" }),
  limit: Type.Optional(Type.Integer({ default: 50, minimum: 1 })),
  map: Type.Optional(FieldMapSchema),
});

export type QmdSource = Type.Static<typeof QmdSourceSchema>;

export default definePlugin(import.meta, {
  kind: "source",
  schema: QmdSourceSchema,
  itemBucketIdentity: (config) => [...config.collections].sort().join(","),
  runtime,
  healthCheck,
});

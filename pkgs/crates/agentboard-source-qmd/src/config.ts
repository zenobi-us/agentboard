import Type from "typebox";

import { definePlugin, FieldMapSchema } from "@agentboard/core/config";

export const QmdSourceSchema = Type.Object({
  collections: Type.Array(Type.String()),
  query: Type.String(),
  limit: Type.Optional(Type.Integer({ default: 50, minimum: 0 })),
  map: Type.Optional(FieldMapSchema),
});

export type QmdSource = Type.Static<typeof QmdSourceSchema>;

export default definePlugin(import.meta, {
  kind: "source",
  schema: QmdSourceSchema,
  itemBucketIdentity: (config) => [...config.collections].sort().join(","),
  runtime: () => ({
    collect: () => Promise.reject(new Error("QMD Bun Source runtime is not available")),
  }),
  healthCheck: () => Promise.reject(new Error("QMD Bun Source runtime is not available")),
});

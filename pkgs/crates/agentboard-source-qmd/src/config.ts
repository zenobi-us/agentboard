import Type from "typebox";

import { FieldMapSchema } from "@agentboard/core/config";

export const QmdSourceSchema = Type.Object({
  kind: Type.Literal("qmd"),
  collections: Type.Array(Type.String()),
  query: Type.String(),
  limit: Type.Optional(Type.Integer({ default: 50, minimum: 0 })),
  map: Type.Optional(FieldMapSchema),
});

export type QmdSource = Type.Static<typeof QmdSourceSchema>;

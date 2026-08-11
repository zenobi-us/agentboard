import Type from "typebox";

export const FieldMapSchema = Type.Object({
  id: Type.Optional(Type.Union([Type.String(), Type.Null()])),
  title: Type.Optional(Type.Union([Type.String(), Type.Null()])),
  status: Type.Optional(Type.Union([Type.String(), Type.Null()])),
  url: Type.Optional(Type.Union([Type.String(), Type.Null()])),
});

export type FieldMap = Type.Static<typeof FieldMapSchema>;

import { app } from "./app.ts";

export const listCmd = app
  .sub("list")
  .meta({ description: "List the latest stored items" })
  .run(() => {
    console.log("list placeholder");
  });

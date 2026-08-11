import { app } from "./app.ts";

export const initCmd = app
  .sub("init")
  .meta({ description: "Create an empty workspace" })
  .run(() => {
    console.log("init placeholder");
  });

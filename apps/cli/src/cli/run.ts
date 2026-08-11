import { app } from "./app.ts";

export const runCmd = app
  .sub("run")
  .meta({ description: "Execute one workspace run" })
  .run(() => {
    console.log("run placeholder");
  });

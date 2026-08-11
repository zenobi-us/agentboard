import { app } from "./app.ts";

export const doctorCmd = app
  .sub("doctor")
  .meta({ description: "Validate the workspace and local environment" })
  .run(() => {
    console.log("doctor placeholder");
  });

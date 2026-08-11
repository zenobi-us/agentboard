import { app } from "./app.ts";

export const dashboardCmd = app
  .sub("dashboard")
  .meta({ description: "Open the read-only store dashboard" })
  .run(() => {
    console.log("dashboard placeholder");
  });

import { app } from "./app.ts";
import { startTui } from "../tui/main.tsx";

export const tuiCmd = app
  .sub("tui")
  .meta({ description: "Open the experimental terminal interface" })
  .run(() => startTui());

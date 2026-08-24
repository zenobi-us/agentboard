import pkg from "../../package.json";
import { app } from "./app.ts";
import { startTui } from "../tui/main.tsx";

export const tuiCmd = app
  .sub("tui")
  .args([{ name: "workspace", type: "string", default: ".agentboard.toml" }])
  .meta({ description: "Open the experimental terminal interface" })
  .run(({ args }) => startTui(args.workspace, pkg.version));

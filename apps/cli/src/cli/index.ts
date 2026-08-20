#!/usr/bin/env bun

import { helpPlugin, versionPlugin } from "@crustjs/plugins";
import pkg from "../../package.json";
import { app } from "./app.ts";
import { dashboardCmd } from "./dashboard.ts";
import { doctorCmd } from "./doctor.ts";
import { initCmd } from "./init.ts";
import { listCmd } from "./list.ts";
import { runCmd } from "./run.ts";
import { schemaCmd } from "./schema.ts";
import { showCmd } from "./show.ts";
import { workspaceCmd, workspacesCmd } from "./workspace.ts";

const globalFlags = new Set(["-v", "--verbose", "-q", "--quiet", "--color", "--log-file"]);
const rawArgs = process.argv.slice(2);
const leadingFlags: string[] = [];
const commandArgs: string[] = [];
for (let index = 0; index < rawArgs.length; index++) {
  const value = rawArgs[index]!;
  if (commandArgs.length === 0 && globalFlags.has(value)) {
    leadingFlags.push(value);
    if (value === "--color" || value === "--log-file") leadingFlags.push(rawArgs[++index]!);
  } else {
    commandArgs.push(value);
  }
}
if (leadingFlags.length > 0) process.argv.splice(2, rawArgs.length, ...commandArgs, ...leadingFlags);

await app
  .use(versionPlugin(pkg.version))
  .use(helpPlugin())
  .command(doctorCmd)
  .command(dashboardCmd)
  .command(initCmd)
  .command(listCmd)
  .command(runCmd)
  .command(schemaCmd)
  .command(showCmd)
  .command(workspaceCmd)
  .command(workspacesCmd)
  .execute();

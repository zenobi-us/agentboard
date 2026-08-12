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

await app
  .use(versionPlugin(pkg.version))
  .use(helpPlugin())
  .command(doctorCmd)
  .command(dashboardCmd)
  .command(initCmd)
  .command(listCmd)
  .command(runCmd)
  .command(schemaCmd)
  .execute();

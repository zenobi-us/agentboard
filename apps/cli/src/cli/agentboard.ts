#!/usr/bin/env bun

import { printLegacyDeprecation } from "./app.ts";

printLegacyDeprecation();
await import("./index.ts");

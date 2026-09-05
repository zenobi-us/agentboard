import { expect, test } from "bun:test";
import { chmodSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

import qmd from "./config.ts";
import { healthCheck, runtime } from "./runtime.ts";

const context = (sourceId = "tasks") => ({
  sourceId,
  cancellation: new AbortController().signal,
});

test("exports one QMD Source Plugin Descriptor", () => {
  expect(qmd.kind).toBe("source");
  expect(qmd.schema).toBeDefined();
  expect(qmd.runtime).toBe(runtime);
  expect(qmd.healthCheck).toBe(healthCheck);
  expect(qmd.itemBucketIdentity!({ collections: ["work", "tasks"], query: "ready", limit: 50 })).toBe("tasks,work");
});

test("normalizes QMD results, maps frontmatter, and preserves raw payload", async () => {
  const bin = mkdtempSync(join(tmpdir(), "agentboard-qmd-test-"));
  const command = join(bin, "qmd");
  const result = {
    path: "/notes/AB-1.md",
    body: "---\nid: AB-1\ntitle: Do it\nstatus: ready\nagentboard:\n  reference: customer-42\n---\nBody",
  };
  writeFileSync(command, `#!/bin/sh\nprintf '%s' '${JSON.stringify([result])}'\n`);
  chmodSync(command, 0o755);
  const previousPath = process.env["PATH"];
  process.env["PATH"] = bin;
  try {
    const items = await runtime({
      collections: ["tasks"],
      query: "status:ready",
      limit: 1,
      map: { id: "agentboard.reference" },
    }, context()).collect();
    expect(items).toEqual([{
      id: "/notes/AB-1.md",
      reference_id: "customer-42",
      title: "Do it",
      status: "ready",
      url: "/notes/AB-1.md",
      source_id: "tasks",
      source_kind: "qmd",
      raw: { qmd: result, frontmatter: { id: "AB-1", title: "Do it", status: "ready", agentboard: { reference: "customer-42" } }, body: "Body" },
    }]);
  } finally {
    if (previousPath === undefined) delete process.env["PATH"];
    else process.env["PATH"] = previousPath;
    rmSync(bin, { recursive: true, force: true });
  }
});

test("rejects duplicate QMD document references", async () => {
  const bin = mkdtempSync(join(tmpdir(), "agentboard-qmd-test-"));
  const command = join(bin, "qmd");
  const body = "---\nid: AB-1\ntitle: Do it\nstatus: ready\n---\nBody";
  const results = [{ path: "/notes/AB-1.md", body }, { path: "/notes/AB-1.md", body }];
  writeFileSync(command, `#!/bin/sh\nprintf '%s' '${JSON.stringify(results)}'\n`);
  chmodSync(command, 0o755);
  const previousPath = process.env["PATH"];
  process.env["PATH"] = bin;
  try {
    await expect(runtime({ collections: ["tasks"], query: "ready" }, context()).collect())
      .rejects.toThrow("duplicate item id /notes/AB-1.md in source tasks");
  } finally {
    if (previousPath === undefined) delete process.env["PATH"];
    else process.env["PATH"] = previousPath;
    rmSync(bin, { recursive: true, force: true });
  }
});

test("health check reports a missing QMD command", async () => {
  const previousPath = process.env["PATH"];
  process.env["PATH"] = "/nonexistent";
  try {
    await expect(healthCheck({ collections: ["tasks"], query: "ready" }, context()))
      .rejects.toThrow("required command qmd not found");
  } finally {
    if (previousPath === undefined) delete process.env["PATH"];
    else process.env["PATH"] = previousPath;
  }
});

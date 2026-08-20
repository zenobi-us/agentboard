import { describe, expect, test } from "bun:test";
import { renderListHuman } from "./list.ts";

const item = {
  id: "one",
  reference_id: "ONE-1",
  title: "One",
  status: "ready",
  url: "https://example.test/one",
  source_id: "issues",
  source_kind: "memory",
  raw: {},
};

describe("human Store list output", () => {
  test("shows missing and ready-empty Snapshots", () => {
    const output = renderListHuman([
      { sourceId: "missing", sourceSlug: "missing-memory", state: "missing", items: [] },
      { sourceId: "empty", sourceSlug: "empty-memory", state: "ready", items: [] },
      { sourceId: "items", sourceSlug: "items-memory", state: "ready", items: [{ item, sourceSlug: "items-memory", actionState: "succeeded" }] },
    ]);
    expect(output).toBe([
      "Source: missing",
      "Snapshot: missing (run successfully to populate it)",
      "",
      "Source: empty",
      "Snapshot: ready (0 items)",
      "",
      "Source: items",
      "Reference ID\tTitle\tStatus\tAction Plan Result",
      "ONE-1\tOne\tready\tsuccess",
      "",
      "",
    ].join("\n"));
  });
});

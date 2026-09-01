import { describe, expect, test } from "bun:test";
import { appendFileSync } from "node:fs";
import { mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { Item } from "@clankpipe/core/config";
import { appendActionAttempt, appendSourceSnapshot, readStoreViews, sourceSlug, successfulActionKeys } from "./store.ts";

type TestSource = {
  id: string;
  packageName: string;
  itemBucketIdentity: string;
  source: { config: unknown };
  actions: readonly unknown[];
};

const item = (id: string, sourceId: string): Item => ({
  id,
  reference_id: id,
  title: id,
  status: "open",
  url: `https://example.test/${id}`,
  source_id: sourceId,
  source_kind: "github",
  raw: {},
});

function workspace(sources: readonly TestSource[]) {
  return { id: "test", path: ".", sources } as never;
}

function source(id: string, config: unknown = {}): TestSource {
  return {
    id,
    packageName: "@clankpipe/source-github",
    itemBucketIdentity: "github.com",
    source: { config },
    actions: [],
  };
}

describe("Store bucket paths", () => {
  test("uses the registered Source kind and Item Bucket identity", () => {
    expect(sourceSlug(source("issues") as never)).toBe("github-github-com-1fee6d0f9c10");
  });

  test("does not truncate long Item Bucket identities", () => {
    const identity = "a".repeat(60);
    expect(sourceSlug({ ...source("issues"), itemBucketIdentity: identity } as never)).toBe(`github-${identity}-2d0d4b1e035f`);
  });
});

describe("Source Snapshot selection", () => {
  test("uses the stable Source ID, kind, and config key", async () => {
    const root = await mkdtemp(join(tmpdir(), "agentboard-store-"));
    const configured = source("issues");
    await appendSourceSnapshot(workspace([configured]), configured as never, [], new AbortController().signal, root);

    const path = join(root, "test", "items-github-github-com-1fee6d0f9c10.snapshots");
    const boundary = JSON.parse((await readFile(path, "utf8")).trim()) as { snapshot_key: string };
    expect(boundary.snapshot_key).toBe("dc7f8a622c44073e");
  });

  test("keys snapshots by Source ID, kind, and config, not Actions", async () => {
    const root = await mkdtemp(join(tmpdir(), "agentboard-store-"));
    const first = { ...source("issues"), actions: [{ id: "one" }] };
    const second = { ...source("issues"), actions: [{ id: "two" }] };
    await appendSourceSnapshot(workspace([first]), first as never, [item("first", "issues")], new AbortController().signal, root);
    await appendSourceSnapshot(workspace([second]), second as never, [item("second", "issues")], new AbortController().signal, root);

    const path = join(root, "test", "items-github-github-com-1fee6d0f9c10.snapshots");
    const boundaries = (await readFile(path, "utf8")).trim().split("\n").map((line) => JSON.parse(line)) as Array<{ snapshot_key: string }>;
    expect(boundaries[0]?.snapshot_key).toBe(boundaries[1]?.snapshot_key);
  });

  test("selects the latest boundary for each configured Source in a shared Item Bucket", async () => {
    const root = await mkdtemp(join(tmpdir(), "agentboard-store-"));
    const first = source("first");
    const second = source("second");
    const ws = workspace([first, second]);
    await appendSourceSnapshot(ws, first as never, [item("from-first", "first")], new AbortController().signal, root);
    await appendSourceSnapshot(ws, second as never, [item("from-second", "second")], new AbortController().signal, root);

    const snapshots = await readStoreViews(ws, root);
    expect(snapshots[0]?.items.map(({ item: value }) => value.id)).toEqual(["from-first"]);
    expect(snapshots[1]?.items.map(({ item: value }) => value.id)).toEqual(["from-second"]);
  });

  test("does not erase a concurrent boundary after cancellation", async () => {
    const root = await mkdtemp(join(tmpdir(), "agentboard-store-"));
    const configured = source("issues");
    const boundaryPath = join(root, "test", "items-github-github-com-1fee6d0f9c10.snapshots");
    let reads = 0;
    const cancellation = {
      get aborted() {
        reads += 1;
        if (reads === 4) appendFileSync(boundaryPath, '{"snapshot_key":"other","snapshot_id":"other"}\n');
        return reads >= 4;
      },
      reason: new Error("cancelled"),
    } as unknown as AbortSignal;

    await appendSourceSnapshot(workspace([configured]), configured as never, [], cancellation, root);
    expect((await readFile(boundaryPath, "utf8")).trim().split("\n")).toHaveLength(2);
  });
});

describe("Action attempt paths", () => {
  test("reads attempts from the configured-Source hash path", async () => {
    const root = await mkdtemp(join(tmpdir(), "agentboard-store-"));
    const configured = {
      ...source("issues"),
      actions: [{ packageName: "@clankpipe/action-run-cmd", config: { command: "echo ok" } }],
    };
    const directory = join(root, "test");
    await mkdir(directory, { recursive: true });
    await writeFile(join(directory, "actions-github-github-com-3aeb00246038-old-plan.jsonl"), `${JSON.stringify({
      outcome: "success",
      source_id: "issues",
      item_id: "item-1",
      source_action_index: 0,
      rendered_action_hash: "old-rendered",
    })}\n`);
    await appendActionAttempt(workspace([configured]), configured as never, {
      ts: new Date().toISOString(),
      outcome: "success",
      stdout: "",
      stderr: "",
      source_id: "issues",
      item_id: "item-1",
      source_action_index: 0,
      uses: "@clankpipe/action-run-cmd",
      rendered_action_hash: "rendered",
    }, root);

    expect(await successfulActionKeys(workspace([configured]), configured as never, root)).toEqual(new Set(["issues\u0000item-1\u00000\u0000rendered"]));
  });
});

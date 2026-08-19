import { expect, test } from "bun:test";

import github from "./config.ts";
import { healthCheck, runtime } from "./runtime.ts";

const context = (sourceId = "issues") => ({
  sourceId,
  cancellation: new AbortController().signal,
});

const issue = {
  number: 42,
  reference_id: "GH-42",
  title: "Fix the source",
  state: "open",
  html_url: "https://github.com/zenobi-us/agentboard/issues/42",
  repository_url: "https://api.github.com/repos/zenobi-us/agentboard",
  labels: [{ name: "ready" }],
};

test("exports one GitHub Source Plugin Descriptor", () => {
  expect(github.kind).toBe("source");
  expect(github.schema).toBeDefined();
  expect(github.runtime).toBe(runtime);
  expect(github.healthCheck).toBe(healthCheck);
  expect(github.itemBucketIdentity!({} as never)).toBe("github.com");
});

test("collects issue items with issue-only query, field maps, status maps, and raw payload", async () => {
  const originalFetch = globalThis.fetch;
  let requestUrl = "";
  globalThis.fetch = (async (input: string | URL) => {
    requestUrl = String(input);
    return new Response(JSON.stringify({ items: [issue] }), { status: 200 });
  }) as unknown as typeof fetch;
  try {
    const items = await runtime({
      query: "repo:zenobi-us/agentboard is:open",
      credentials: { helper: "printf token" },
      field_map: { id: "reference_id", title: "title", status: "state", url: "html_url" },
      status_map: { ready: "in-progress", open: "open" },
      limit: 1,
    } as never, context()).collect();
    expect(items).toEqual([{
      id: "zenobi-us/agentboard#42",
      reference_id: "GH-42",
      title: "Fix the source",
      status: "in-progress",
      url: "https://github.com/zenobi-us/agentboard/issues/42",
      source_id: "issues",
      source_kind: "github",
      raw: { github: { issue } },
    }]);
    expect(requestUrl).toContain("q=is%3Aissue%20repo%3Azenobi-us%2Fagentboard%20is%3Aopen");
    expect(requestUrl).toContain("per_page=1");
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("maps GitHub state when no label matches", async () => {
  const originalFetch = globalThis.fetch;
  globalThis.fetch = (async () => new Response(JSON.stringify({ items: [{ ...issue, labels: [] }] }), { status: 200 })) as unknown as typeof fetch;
  try {
    const items = await runtime({ query: "repo:x/y", credentials: { helper: "printf token" }, status_map: { open: "ready" }, limit: 1 } as never, context()).collect();
    expect(items[0]?.status).toBe("ready");
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("rejects pull requests and duplicate item ids", async () => {
  const originalFetch = globalThis.fetch;
  globalThis.fetch = (async () => new Response(JSON.stringify({ items: [
    { ...issue, pull_request: { url: "https://api.github.com/pulls/42" } },
  ] }), { status: 200 })) as unknown as typeof fetch;
  try {
    await expect(runtime({ query: "repo:x/y", credentials: { helper: "printf token" }, status_map: { open: "open" } } as never, context()).collect())
      .rejects.toThrow("github issue search returned pull request");
  } finally {
    globalThis.fetch = originalFetch;
  }

  globalThis.fetch = (async () => new Response(JSON.stringify({ items: [issue, issue] }), { status: 200 })) as unknown as typeof fetch;
  try {
    await expect(runtime({ query: "repo:x/y", credentials: { helper: "printf token" }, status_map: { ready: "ready" }, limit: 2 } as never, context()).collect())
      .rejects.toThrow("duplicate item id zenobi-us/agentboard#42 in source issues");
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("health check validates the credential with GitHub", async () => {
  const originalFetch = globalThis.fetch;
  let requestUrl = "";
  globalThis.fetch = (async (input: string | URL) => {
    requestUrl = String(input);
    return new Response(JSON.stringify({ items: [] }), { status: 200 });
  }) as unknown as typeof fetch;
  try {
    await healthCheck({ query: "repo:x/y", credentials: { helper: "printf token" }, status_map: { open: "open" } } as never, context("github"));
    expect(requestUrl).toContain("q=is%3Aissue%20repo%3Ax%2Fy");
    expect(requestUrl).toContain("per_page=1");
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("health check reports GitHub API errors", async () => {
  const originalFetch = globalThis.fetch;
  globalThis.fetch = (async () => new Response("bad token", { status: 401 })) as unknown as typeof fetch;
  try {
    await expect(healthCheck({ query: "repo:x/y", credentials: { helper: "printf token" }, status_map: { open: "open" } } as never, context()))
      .rejects.toThrow("github issue search failed with 401");
  } finally {
    globalThis.fetch = originalFetch;
  }
});

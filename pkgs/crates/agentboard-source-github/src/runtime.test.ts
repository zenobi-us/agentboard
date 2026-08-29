import { expect, test } from "bun:test";

import github from "./config.ts";
import { healthCheck, runtime } from "./runtime.ts";

const context = (sourceId = "issues") => ({
  sourceId,
  cancellation: new AbortController().signal,
});

const issue = {
  number: 42,
  title: "Fix the source",
  state: "OPEN",
  url: "https://github.com/zenobi-us/agentboard/issues/42",
  repository: { nameWithOwner: "zenobi-us/agentboard" },
  labels: { nodes: [{ name: "ready" }] },
};

const graphqlResponse = (nodes: unknown[], hasNextPage = false, endCursor: string | null = null) => ({
  data: { search: { nodes, pageInfo: { hasNextPage, endCursor } } },
});

test("exports one GitHub Source Plugin Descriptor", () => {
  expect(github.kind).toBe("source");
  expect(github.schema).toBeDefined();
  expect(github.runtime).toBe(runtime);
  expect(github.healthCheck).toBe(healthCheck);
  expect(github.itemBucketIdentity!({} as never)).toBe("github.com");
});

test("collects issue items through GraphQL advanced search", async () => {
  const originalFetch = globalThis.fetch;
  let request: { url: string; init?: RequestInit } | undefined;
  globalThis.fetch = (async (input: string | URL, init?: RequestInit) => {
    request = { url: String(input), init };
    return new Response(JSON.stringify(graphqlResponse([issue])), { status: 200 });
  }) as unknown as typeof fetch;
  try {
    const items = await runtime({
      query: 'repo:zenobi-us/agentboard is:open (label:"ready" OR label:"blocked")',
      credentials: { helper: "printf token" },
      field_map: { id: "repository.nameWithOwner", title: "title", status: "state", url: "url" },
      status_map: { ready: "in-progress", OPEN: "open" },
      limit: 1,
    } as never, context()).collect();
    expect(items).toEqual([{
      id: "zenobi-us/agentboard#42",
      reference_id: "zenobi-us/agentboard",
      title: "Fix the source",
      status: "in-progress",
      url: "https://github.com/zenobi-us/agentboard/issues/42",
      source_id: "issues",
      source_kind: "github",
      raw: { github: { issue } },
    }]);
    expect(request?.url).toBe("https://api.github.com/graphql");
    const body = JSON.parse(String(request?.init?.body)) as { query: string; variables: { query: string; limit: number; after?: string | null } };
    expect(body.query).toContain("ISSUE_ADVANCED");
    expect(body.variables.query).toBe('(repo:zenobi-us/agentboard is:open (label:"ready" OR label:"blocked")) state:open type:issue');
    expect(body.variables.limit).toBe(1);
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("maps GitHub state when no label matches", async () => {
  const originalFetch = globalThis.fetch;
  globalThis.fetch = (async () => new Response(JSON.stringify(graphqlResponse([{ ...issue, labels: { nodes: [] } }])), { status: 200 })) as unknown as typeof fetch;
  try {
    const items = await runtime({ query: "repo:x/y", credentials: { helper: "printf token" }, status_map: { OPEN: "ready" }, limit: 1 } as never, context()).collect();
    expect(items[0]?.status).toBe("ready");
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("rejects pull requests and duplicate item ids", async () => {
  const originalFetch = globalThis.fetch;
  globalThis.fetch = (async () => new Response(JSON.stringify(graphqlResponse([
    { ...issue, pull_request: { url: "https://api.github.com/pulls/42" } },
  ])), { status: 200 })) as unknown as typeof fetch;
  try {
    await expect(runtime({ query: "repo:x/y", credentials: { helper: "printf token" }, status_map: { OPEN: "open" } } as never, context()).collect())
      .rejects.toThrow("github issue search returned pull request");
  } finally {
    globalThis.fetch = originalFetch;
  }

  globalThis.fetch = (async () => new Response(JSON.stringify(graphqlResponse([issue, issue])), { status: 200 })) as unknown as typeof fetch;
  try {
    await expect(runtime({ query: "repo:x/y", credentials: { helper: "printf token" }, status_map: { ready: "ready" }, limit: 2 } as never, context()).collect())
      .rejects.toThrow("duplicate item id zenobi-us/agentboard#42 in source issues");
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("follows GraphQL search cursors", async () => {
  const originalFetch = globalThis.fetch;
  const requests: Array<{ variables: { after?: string | null } }> = [];
  globalThis.fetch = (async (_input: string | URL, init?: RequestInit) => {
    const body = JSON.parse(String(init?.body)) as { variables: { after?: string | null } };
    requests.push(body);
    return new Response(JSON.stringify(requests.length === 1
      ? graphqlResponse([issue], true, "cursor-1")
      : graphqlResponse([{ ...issue, number: 43 }])), { status: 200 });
  }) as unknown as typeof fetch;
  try {
    const items = await runtime({ query: "repo:x/y", credentials: { helper: "printf token" }, status_map: { OPEN: "ready" }, limit: 2 } as never, context()).collect();
    expect(items).toHaveLength(2);
    expect(requests.map(({ variables }) => variables.after)).toEqual([undefined, "cursor-1"]);
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("health check validates the credential with GitHub", async () => {
  const originalFetch = globalThis.fetch;
  let request: { url: string; init?: RequestInit } | undefined;
  globalThis.fetch = (async (input: string | URL, init?: RequestInit) => {
    request = { url: String(input), init };
    return new Response(JSON.stringify(graphqlResponse([])), { status: 200 });
  }) as unknown as typeof fetch;
  try {
    await healthCheck({ query: "repo:x/y", credentials: { helper: "printf token" }, status_map: { OPEN: "open" } } as never, context("github"));
    expect(request?.url).toBe("https://api.github.com/graphql");
    expect(String(request?.init?.body)).toContain('"limit":1');
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("health check reports GitHub API errors", async () => {
  const originalFetch = globalThis.fetch;
  globalThis.fetch = (async () => new Response("bad token", { status: 401 })) as unknown as typeof fetch;
  try {
    await expect(healthCheck({ query: "repo:x/y", credentials: { helper: "printf token" }, status_map: { OPEN: "open" } } as never, context()))
      .rejects.toThrow("github issue search failed with 401");
  } finally {
    globalThis.fetch = originalFetch;
  }
});

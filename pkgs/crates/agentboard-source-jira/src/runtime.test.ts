import { expect, test } from "bun:test";
import { healthCheck, runtime } from "./runtime.ts";
import jira from "./config.ts";

test("exports one Jira Source Plugin Descriptor", () => {
  expect(jira.kind).toBe("source");
  expect(jira.schema).toBeDefined();
  expect(jira.runtime).toBe(runtime);
  expect(jira.healthCheck).toBe(healthCheck);
});

test("normalizes Jira issues, maps fields, requests inferred fields, and preserves raw payload", async () => {
  const originalFetch = globalThis.fetch;
  let request: Record<string, unknown> | undefined;
  const issue = {
    id: "10001",
    key: "AB-1",
    fields: {
      summary: "Do it",
      status: { name: "To Do" },
      customer: "customer-42",
    },
  };
  globalThis.fetch = (async (_input: unknown, init?: RequestInit) => {
    request = JSON.parse(String(init?.body)) as Record<string, unknown>;
    return new Response(JSON.stringify({ issues: [issue] }), { status: 200 });
  }) as unknown as typeof fetch;
  const previousEmail = process.env["JIRA_EMAIL"];
  const previousToken = process.env["JIRA_API_TOKEN"];
  process.env["JIRA_EMAIL"] = "user@example.com";
  process.env["JIRA_API_TOKEN"] = "secret";
  try {
    const items = await runtime({
      site: "https://example.atlassian.net",
      jql: "project = AB",
      fields: ["assignee"],
      field_map: { id: "fields.customer", title: "fields.summary", status: "fields.status.name" },
      status_map: { "To Do": "ready" },
      limit: 1,
    } as never, { sourceId: "issues", cancellation: new AbortController().signal }).collect();
    expect(items).toEqual([{
      id: "10001",
      reference_id: "customer-42",
      title: "Do it",
      status: "ready",
      url: "https://example.atlassian.net/browse/AB-1",
      source_id: "issues",
      source_kind: "jira",
      raw: { jira: issue },
    }]);
    expect(request).toEqual({
      jql: "project = AB",
      maxResults: 1,
      fields: ["summary", "status", "customer", "assignee"],
    });
  } finally {
    globalThis.fetch = originalFetch;
    if (previousEmail === undefined) delete process.env["JIRA_EMAIL"];
    else process.env["JIRA_EMAIL"] = previousEmail;
    if (previousToken === undefined) delete process.env["JIRA_API_TOKEN"];
    else process.env["JIRA_API_TOKEN"] = previousToken;
  }
});

test("rejects duplicate Jira item identities", async () => {
  const originalFetch = globalThis.fetch;
  globalThis.fetch = (async () => new Response(JSON.stringify({ issues: [
    { id: "10001", key: "AB-1", fields: { summary: "One", status: { name: "Ready" } } },
    { id: "10001", key: "AB-2", fields: { summary: "Two", status: { name: "Ready" } } },
  ] }), { status: 200 })) as unknown as typeof fetch;
  const previousEmail = process.env["JIRA_EMAIL"];
  const previousToken = process.env["JIRA_API_TOKEN"];
  process.env["JIRA_EMAIL"] = "user@example.com";
  process.env["JIRA_API_TOKEN"] = "secret";
  try {
    await expect(runtime({ site: "https://example.atlassian.net", jql: "project = AB", limit: 2 } as never,
      { sourceId: "issues", cancellation: new AbortController().signal }).collect())
      .rejects.toThrow("duplicate item id 10001 in source issues");
  } finally {
    globalThis.fetch = originalFetch;
    if (previousEmail === undefined) delete process.env["JIRA_EMAIL"];
    else process.env["JIRA_EMAIL"] = previousEmail;
    if (previousToken === undefined) delete process.env["JIRA_API_TOKEN"];
    else process.env["JIRA_API_TOKEN"] = previousToken;
  }
});

test("health check rejects incomplete credential helper output", async () => {
  await expect(healthCheck({ site: "https://example.atlassian.net", credentials: { helper: "printf 'username=user\\n'" } } as never,
    { sourceId: "jira", cancellation: new AbortController().signal }))
    .rejects.toThrow("credential helper missing token");
});

test("passes the Git credential request to the helper", async () => {
  const originalFetch = globalThis.fetch;
  globalThis.fetch = (async () => new Response(JSON.stringify({ issues: [] }), { status: 200 })) as unknown as typeof fetch;
  try {
    const config = {
      site: "https://example.atlassian.net",
      jql: "project = AB",
      credentials: {
        helper: "input=$(cat); test \"$(printf '%s' \"$input\" | sed -n '1p')\" = protocol=https && test \"$(printf '%s' \"$input\" | sed -n '2p')\" = host=example.atlassian.net && printf 'username=user\\npassword=token\\n'",
      },
      limit: 1,
    } as never;
    const items = await runtime(config, { sourceId: "jira", cancellation: new AbortController().signal }).collect();
    expect(items).toEqual([]);
  } finally {
    globalThis.fetch = originalFetch;
  }
});

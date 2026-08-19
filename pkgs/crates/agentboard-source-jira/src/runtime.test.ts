import { expect, test } from "bun:test";
import { runtime } from "./runtime.ts";

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

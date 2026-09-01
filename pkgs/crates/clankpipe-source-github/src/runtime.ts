import type { Item, SourceRuntime } from "@clankpipe/core/config";
import type { GithubSource } from "./config.ts";

function stopProcessGroup(child: Bun.Subprocess): void {
  if (process.platform === "win32") {
    // Bun has no portable process-group signal on Windows.
    child.kill();
    return;
  }
  try { process.kill(-child.pid, "SIGTERM"); } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ESRCH") throw error;
  }
}

async function helper(command: string, signal: AbortSignal): Promise<string> {
  if (signal.aborted) throw new Error("github operation cancelled");
  const child = Bun.spawn(["sh", "-c", command], { stdout: "pipe", stderr: "pipe", detached: true });
  const stop = () => stopProcessGroup(child);
  signal.addEventListener("abort", stop, { once: true });
  try {
    const code = await child.exited;
    const [stdout, stderr] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text()]);
    if (signal.aborted) throw new Error("github operation cancelled");
    if (code) throw new Error(`credential helper failed with ${code}: ${stderr}`);
    const token = stdout.trim();
    if (!token) throw new Error("credential helper returned an empty token");
    return token;
  } finally { signal.removeEventListener("abort", stop); }
}

const get = (value: any, path: string): string => { let current = value; for (const part of path.split(".")) current = current?.[part]; if (typeof current !== "string") throw new Error(`github field_map ${path} must resolve to a string`); return current; };

const searchDocument = `
  query SearchIssues($query: String!, $limit: Int!, $after: String) {
    search(type: ISSUE_ADVANCED, query: $query, first: $limit, after: $after) {
      nodes {
        ... on Issue {
          number
          title
          url
          state
          repository { nameWithOwner }
          labels(first: 100) { nodes { name } }
        }
      }
      pageInfo { hasNextPage endCursor }
    }
  }
`;

function issueQuery(query: string): string {
  return `(${query}) state:open type:issue`;
}

async function search(config: GithubSource, token: string, signal: AbortSignal, limit: number, after?: string): Promise<Response> {
  return fetch("https://api.github.com/graphql", {
    method: "POST",
    headers: { authorization: `Bearer ${token}`, accept: "application/vnd.github+json", "content-type": "application/json", "user-agent": "clankpipe" },
    body: JSON.stringify({ query: searchDocument, variables: { query: issueQuery(config.query), limit, after } }),
    signal,
  });
}

export function runtime(config: GithubSource, context: { sourceId: string; cancellation: AbortSignal }): SourceRuntime {
  return { collect: async () => {
    const token = await helper(config.credentials.helper, context.cancellation);
    const items: Item[] = [];
    const ids = new Set<string>();
    let after: string | undefined;
    while (items.length < (config.limit ?? 50)) {
      if (context.cancellation.aborted) throw new Error("github operation cancelled");
      const pageSize = Math.min((config.limit ?? 50) - items.length, 100);
      const response = await search(config, token, context.cancellation, pageSize, after);
      if (!response.ok) throw new Error(`github issue search failed with ${response.status}: ${await response.text()}`);
      const data = await response.json() as any;
      if (Array.isArray(data.errors) && data.errors.length > 0) throw new Error(`github issue search failed: ${data.errors.map((error: any) => error.message).join("; ")}`);
      const searchResult = data.data?.search;
      if (!searchResult || !Array.isArray(searchResult.nodes) || !searchResult.pageInfo) throw new Error("github issue search response missing search results");
      if (searchResult.nodes.length === 0) break;
      for (const issue of searchResult.nodes) {
        if (issue.pull_request) throw new Error("github issue search returned pull request");
        const map = config.field_map ?? {};
        const repo = get(issue, "repository.nameWithOwner");
        const mappedIssue = { ...issue, html_url: issue.url, repository_url: `https://api.github.com/repos/${repo}`, labels: issue.labels?.nodes ?? [] };
        const id = `${repo}#${issue.number}`;
        if (ids.has(id)) throw new Error(`duplicate item id ${id} in source ${context.sourceId}`);
        ids.add(id);
        const state = get(mappedIssue, map.status ?? "state");
        const label = (mappedIssue.labels ?? []).find((label: any) => typeof label?.name === "string" && config.status_map[label.name]);
        const fallbackState = state.toLowerCase();
        items.push({ id, reference_id: map.id ? get(mappedIssue, map.id) : String(issue.number), title: get(mappedIssue, map.title ?? "title"), status: label ? config.status_map[label.name] ?? fallbackState : config.status_map[state] ?? config.status_map[fallbackState] ?? fallbackState, url: get(mappedIssue, map.url ?? "html_url"), source_id: context.sourceId, source_kind: "github", raw: { github: { issue } } });
        if (items.length >= (config.limit ?? 50)) break;
      }
      if (items.length >= (config.limit ?? 50) || !searchResult.pageInfo.hasNextPage) break;
      after = searchResult.pageInfo.endCursor;
      if (!after) break;
    }
    return items;
  }};
}

export async function healthCheck(config: GithubSource, context: { sourceId: string; cancellation: AbortSignal }): Promise<void> {
  const token = await helper(config.credentials.helper, context.cancellation);
  const response = await search(config, token, context.cancellation, 1);
  if (!response.ok) throw new Error(`github issue search failed with ${response.status}: ${await response.text()}`);
  const data = await response.json() as any;
  if (Array.isArray(data.errors) && data.errors.length > 0) throw new Error(`github issue search failed: ${data.errors.map((error: any) => error.message).join("; ")}`);
  if (!data.data?.search) throw new Error("github issue search response missing search results");
}

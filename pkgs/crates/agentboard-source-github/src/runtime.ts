import type { Item, SourceRuntime } from "@agentboard/core/config";
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

function issueQuery(query: string): string {
  return query.split(/\s+/).includes("is:issue") ? query : `is:issue ${query}`;
}

async function search(config: GithubSource, token: string, signal: AbortSignal, limit: number, page = 1): Promise<Response> {
  const query = issueQuery(config.query);
  return fetch(`https://api.github.com/search/issues?q=${encodeURIComponent(query)}&per_page=${limit}&page=${page}`, { headers: { authorization: `Bearer ${token}`, accept: "application/vnd.github+json", "X-GitHub-Api-Version": "2022-11-28", "user-agent": "agentboard" }, signal });
}

export function runtime(config: GithubSource, context: { sourceId: string; cancellation: AbortSignal }): SourceRuntime {
  return { collect: async () => {
    const token = await helper(config.credentials.helper, context.cancellation);
    const items: Item[] = [];
    const ids = new Set<string>();
    for (let page = 1; items.length < (config.limit ?? 50); page += 1) {
      if (context.cancellation.aborted) throw new Error("github operation cancelled");
      const pageSize = Math.min((config.limit ?? 50) - items.length, 100);
      const response = await search(config, token, context.cancellation, pageSize, page);
      if (!response.ok) throw new Error(`github issue search failed with ${response.status}: ${await response.text()}`);
      const data = await response.json() as any;
      if (!Array.isArray(data.items)) throw new Error("github issue search response missing items array");
      if (data.items.length === 0) break;
      for (const issue of data.items) {
        if (issue.pull_request) throw new Error("github issue search returned pull request");
        const map = config.field_map ?? {};
        const repo = get(issue, "repository_url").replace("https://api.github.com/repos/", "");
        const id = `${repo}#${issue.number}`;
        if (ids.has(id)) throw new Error(`duplicate item id ${id} in source ${context.sourceId}`);
        ids.add(id);
        const state = get(issue, map.status ?? "state");
        const label = (issue.labels ?? []).find((label: any) => typeof label?.name === "string" && config.status_map[label.name]);
        items.push({ id, reference_id: map.id ? get(issue, map.id) : String(issue.number), title: get(issue, map.title ?? "title"), status: label ? config.status_map[label.name] ?? state : config.status_map[state] ?? state, url: get(issue, map.url ?? "html_url"), source_id: context.sourceId, source_kind: "github", raw: { github: { issue } } });
        if (items.length >= (config.limit ?? 50)) break;
      }
      if (items.length >= (config.limit ?? 50)) break;
    }
    return items;
  }};
}

export async function healthCheck(config: GithubSource, context: { sourceId: string; cancellation: AbortSignal }): Promise<void> {
  const token = await helper(config.credentials.helper, context.cancellation);
  const response = await search(config, token, context.cancellation, 1);
  if (!response.ok) throw new Error(`github issue search failed with ${response.status}: ${await response.text()}`);
}

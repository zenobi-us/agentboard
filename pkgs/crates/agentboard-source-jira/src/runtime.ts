import type { Item, SourceRuntime } from "@agentboard/core/config";
import type { JiraSource } from "./config.ts";

const get = (value: any, path: string): string => {
  let current = value;
  for (const part of path.split(".")) current = current?.[part];
  if (typeof current !== "string") throw new Error(`jira mapping ${path} must resolve to a string`);
  return current;
};

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

async function command(command: string, site: string, signal: AbortSignal): Promise<string> {
  if (signal.aborted) throw new Error("jira operation cancelled");
  const child = Bun.spawn(["sh", "-c", command], { stdin: "pipe", stdout: "pipe", stderr: "pipe", detached: true });
  const stop = () => stopProcessGroup(child);
  signal.addEventListener("abort", stop, { once: true });
  try {
    child.stdin.write(`protocol=https\nhost=${new URL(site).host}\n\n`);
    child.stdin.end();
    const code = await child.exited;
    const [stdout, stderr] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text()]);
    if (signal.aborted) throw new Error("jira operation cancelled");
    if (code) throw new Error(`credential helper failed with ${code}: ${stderr}`);
    return stdout;
  } finally { signal.removeEventListener("abort", stop); }
}

function parseCredentials(output: string): { username: string; token: string } {
  let username: string | undefined;
  let token: string | undefined;
  for (const line of output.split(/\r?\n/)) {
    const separator = line.indexOf("=");
    if (separator < 0) continue;
    const key = line.slice(0, separator);
    const value = line.slice(separator + 1);
    if (key === "username" || key === "email") username = value;
    if (key === "password" || key === "token") token = value;
  }
  if (!username) throw new Error("credential helper missing username");
  if (!token) throw new Error("credential helper missing token");
  return { username, token };
}

function fields(config: JiraSource): string[] {
  const result = new Set(["summary", "status"]);
  for (const path of Object.values(config.field_map ?? {})) {
    if (path?.startsWith("fields.")) result.add(path.slice("fields.".length).split(".")[0]!);
  }
  for (const field of config.fields ?? []) result.add(field);
  return [...result];
}

export function runtime(config: JiraSource, context: { sourceId: string; cancellation: AbortSignal }): SourceRuntime {
  return { collect: async () => {
    if (context.cancellation.aborted) throw new Error("jira operation cancelled");
    const site = config.site.replace(/\/$/, "");
    const credentials = config.credentials
      ? parseCredentials(await command(config.credentials.helper, site, context.cancellation))
      : {
          username: process.env[config.email_env ?? "JIRA_EMAIL"] ?? "",
          token: process.env[config.token_env ?? "JIRA_API_TOKEN"] ?? "",
        };
    if (!credentials.username) throw new Error(`Jira credential environment variable ${config.email_env ?? "JIRA_EMAIL"} is not configured`);
    if (!credentials.token) throw new Error(`Jira credential environment variable ${config.token_env ?? "JIRA_API_TOKEN"} is not configured`);
    const { username, token } = credentials;
    const items: Item[] = [];
    const ids = new Set<string>();
    let nextPageToken: string | undefined;
    while (items.length < (config.limit ?? 50)) {
      if (context.cancellation.aborted) throw new Error("jira operation cancelled");
      const pageSize = Math.min((config.limit ?? 50) - items.length, 100);
      const body: Record<string, unknown> = { jql: config.jql, maxResults: pageSize, fields: fields(config) };
      if (nextPageToken) body["nextPageToken"] = nextPageToken;
      const response = await fetch(`${site}/rest/api/3/search/jql`, { method: "POST", headers: { authorization: `Basic ${btoa(`${username}:${token}`)}`, "content-type": "application/json" }, body: JSON.stringify(body), signal: context.cancellation });
      if (!response.ok) throw new Error(`jira search failed with ${response.status}: ${await response.text()}`);
      const data = await response.json() as any;
      const issues = data.issues;
      if (!Array.isArray(issues)) throw new Error("jira search response missing issues array");
      if (issues.length === 0) break;
      for (const issue of issues) {
        const map = config.field_map ?? {};
        const key = get(issue, "key");
        const id = get(issue, "id");
        if (ids.has(id)) throw new Error(`duplicate item id ${id} in source ${context.sourceId}`);
        ids.add(id);
        const status = get(issue, map.status ?? "fields.status.name");
        items.push({ id, reference_id: map.id ? get(issue, map.id) : key, title: get(issue, map.title ?? "fields.summary"), status: config.status_map?.[status] ?? status, url: map.url ? get(issue, map.url) : `${site}/browse/${key}`, source_id: context.sourceId, source_kind: "jira", raw: { jira: issue } });
        if (items.length >= (config.limit ?? 50)) break;
      }
      if (items.length >= (config.limit ?? 50)) break;
      const tokenValue = typeof data.nextPageToken === "string" && data.nextPageToken ? data.nextPageToken : undefined;
      if (!tokenValue || tokenValue === nextPageToken) break;
      nextPageToken = tokenValue;
    }
    return items;
  }};
}

export async function healthCheck(config: JiraSource, context: { sourceId: string; cancellation: AbortSignal }): Promise<void> {
  if (config.credentials) {
    parseCredentials(await command(config.credentials.helper, config.site, context.cancellation));
    return;
  }
  if (!process.env[config.email_env ?? "JIRA_EMAIL"] || !process.env[config.token_env ?? "JIRA_API_TOKEN"]) {
    throw new Error("Jira credentials are not configured");
  }
}

import type { Item, SourceRuntime } from "@agentboard/core/config";
import type { QmdSource } from "./config.ts";

function field(value: unknown, path: string, name: string): string {
  let current: unknown = value;
  for (const part of path.split(".")) current = (current as Record<string, unknown> | undefined)?.[part];
  if (typeof current !== "string") throw new Error(`qmd mapping ${name}=${path} must resolve to a string`);
  return current;
}

function parseFrontmatter(body: string): [Record<string, unknown>, string] {
  const match = /^---\n([\s\S]*?)\n---\n([\s\S]*)$/.exec(body);
  if (!match) throw new Error("missing YAML frontmatter");
  return [Bun.YAML.parse(match[1]!) as Record<string, unknown>, match[2]!];
}

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

export function runtime(config: QmdSource, context: { sourceId: string; cancellation: AbortSignal }): SourceRuntime {
  return { collect: async () => {
    if (context.cancellation.aborted) throw new Error("qmd operation cancelled");
    const args = ["query", config.query, "--format", "json", "--full", "-n", String(config.limit ?? 50)];
    for (const collection of config.collections) args.push("-c", collection);
    if (context.cancellation.aborted) throw new Error("qmd operation cancelled");
    const child = Bun.spawn(["qmd", ...args], { stdout: "pipe", stderr: "pipe", detached: true });
    const stop = () => stopProcessGroup(child);
    context.cancellation.addEventListener("abort", stop, { once: true });
    try {
      const code = await child.exited;
      const [stdout, stderr] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text()]);
      if (context.cancellation.aborted) throw new Error("qmd operation cancelled");
      if (code !== 0) throw new Error(`qmd query failed: ${stderr}`);
      const value = JSON.parse(stdout) as unknown;
      const object = value as Record<string, unknown>;
      const results = Array.isArray(value) ? value : object["results"] ?? object["documents"] ?? object["items"];
      if (!Array.isArray(results)) throw new Error("qmd query JSON must be an array or contain results/documents/items");
      const ids = new Set<string>();
      return results.map((result: any): Item => {
        const ref = result.docid ?? result.doc_id ?? result.id ?? result.uri ?? result.path;
        if (typeof ref !== "string" || typeof result.body !== "string") throw new Error("qmd result missing document reference or body");
        const [frontmatter, text] = parseFrontmatter(result.body);
        const map = config.map ?? {};
        const item: Item = { id: ref, reference_id: field(frontmatter, map.id ?? "id", "id"), title: field(frontmatter, map.title ?? "title", "title"), status: field(frontmatter, map.status ?? "status", "status"), url: map.url ? field(frontmatter, map.url, "url") : (typeof frontmatter["url"] === "string" ? frontmatter["url"] : ref), source_id: context.sourceId, source_kind: "qmd", raw: { qmd: result, frontmatter, body: text } };
        if (ids.has(item.id)) throw new Error(`duplicate item id ${item.id} in source ${context.sourceId}`);
        ids.add(item.id);
        return item;
      });
    } finally { context.cancellation.removeEventListener("abort", stop); }
  }};
}

export async function healthCheck(_config: QmdSource, context: { cancellation: AbortSignal }): Promise<void> {
  if (context.cancellation.aborted) throw new Error("qmd operation cancelled");
  const child = Bun.spawn(["qmd", "--version"], { stdout: "ignore", stderr: "ignore", detached: true });
  const stop = () => stopProcessGroup(child);
  context.cancellation.addEventListener("abort", stop, { once: true });
  try {
    const code = await child.exited;
    if (context.cancellation.aborted) throw new Error("qmd operation cancelled");
    if (code !== 0) throw new Error(`required command qmd returned ${code}`);
  } finally { context.cancellation.removeEventListener("abort", stop); }
}

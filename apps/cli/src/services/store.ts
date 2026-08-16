import { createHash } from "node:crypto";
import { appendFile, mkdir, open, readFile, rename, writeFile } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";

import type { ActionResult, Item } from "@agentboard/core/config";

import type { LoadedWorkspace, LoadedWorkspaceSource } from "./config/workspace.ts";

const SNAPSHOT_KEY_FIELD = "_agentboard_snapshot_key";
const SNAPSHOT_ID_FIELD = "_agentboard_snapshot_id";

export type CollectionState = "collecting" | "complete" | "failed" | "cancelled";

export interface SourceCollectionStatus {
  readonly state: CollectionState;
  readonly updated_at: string;
  readonly error?: string;
}

export interface ActionAttempt extends ActionResult {
  readonly ts: string;
  readonly source_id: string;
  readonly item_id: string;
  readonly source_action_index: number;
  readonly uses: string;
  readonly rendered_action_hash: string;
}

export function workspaceStoreRoot(workspace: LoadedWorkspace, root?: string): string {
  if (root) return join(root, workspace.id);
  const dataHome = process.env["XDG_DATA_HOME"] ?? join(homedir(), ".local", "share");
  return join(dataHome, "agentboard", workspace.id);
}

export async function acquireWorkspaceLock(
  workspace: LoadedWorkspace,
  root?: string,
): Promise<() => Promise<void>> {
  const directory = workspaceStoreRoot(workspace, root);
  const path = join(directory, "run.lock");
  await mkdir(directory, { recursive: true });
  const handle = await open(path, "a+");
  let unlock: () => void;
  try {
    unlock = await acquireNativeLock(handle.fd);
  } catch (error) {
    await handle.close();
    throw new Error(`workspace lock is held at ${path}: ${String(error)}`);
  }
  return async () => {
    unlock();
    await handle.close();
  };
}

export async function setSourceCollectionStatus(
  workspace: LoadedWorkspace,
  sourceId: string,
  state: CollectionState,
  error?: string,
  root?: string,
): Promise<void> {
  const directory = join(workspaceStoreRoot(workspace, root), "sources", sourceIdSlug(sourceId));
  const path = join(directory, "collection-status.json");
  const temporary = `${path}.${process.pid}.${crypto.randomUUID()}.tmp`;
  await mkdir(directory, { recursive: true });
  const status: SourceCollectionStatus = {
    state,
    updated_at: new Date().toISOString(),
    ...(error === undefined ? {} : { error }),
  };
  await writeFile(temporary, JSON.stringify(status, null, 2));
  await rename(temporary, path);
}

export async function appendSourceSnapshot(
  workspace: LoadedWorkspace,
  source: LoadedWorkspaceSource,
  items: readonly Item[],
  cancellation: AbortSignal,
  root?: string,
): Promise<void> {
  const storeRoot = workspaceStoreRoot(workspace, root);
  const slug = sourceSlug(source);
  const itemPath = join(storeRoot, `items-${slug}.jsonl`);
  const boundaryPath = join(storeRoot, `items-${slug}.snapshots`);
  const snapshotKey = shortHash(stableJson({
    id: source.id,
    uses: source.packageName,
    config: source.source.config,
  }));
  const snapshotId = `${Date.now()}-${crypto.randomUUID()}`;
  throwIfCancelled(cancellation);
  await mkdir(storeRoot, { recursive: true });
  throwIfCancelled(cancellation);
  if (items.length > 0) {
    const records = items.map((item) => JSON.stringify({
      ...item,
      [SNAPSHOT_KEY_FIELD]: snapshotKey,
      [SNAPSHOT_ID_FIELD]: snapshotId,
    })).join("\n") + "\n";
    await appendFile(itemPath, records);
  } else {
    await appendFile(itemPath, "");
  }
  throwIfCancelled(cancellation);
  await appendFile(boundaryPath, `${JSON.stringify({ snapshot_key: snapshotKey, snapshot_id: snapshotId })}\n`);
}

export async function successfulActionKeys(
  workspace: LoadedWorkspace,
  source: LoadedWorkspaceSource,
  root?: string,
): Promise<Set<string>> {
  const path = actionPath(workspace, source, root);
  const attempts = await readJsonLines<ActionAttempt>(path);
  const latest = new Map<string, ActionAttempt["outcome"]>();
  for (const attempt of attempts) {
    latest.set(actionKey(
      attempt.source_id,
      attempt.item_id,
      attempt.source_action_index,
      attempt.rendered_action_hash,
    ), attempt.outcome);
  }
  return new Set([...latest]
    .filter(([, outcome]) => outcome === "success")
    .map(([key]) => key));
}

export async function appendActionAttempt(
  workspace: LoadedWorkspace,
  source: LoadedWorkspaceSource,
  attempt: ActionAttempt,
  root?: string,
): Promise<void> {
  const path = actionPath(workspace, source, root);
  await mkdir(workspaceStoreRoot(workspace, root), { recursive: true });
  await appendFile(path, `${JSON.stringify(attempt)}\n`);
}

export function renderedActionHash(uses: string, inputs: unknown): string {
  return hash(stableJson({ uses, with: inputs }));
}

export function actionKey(
  sourceId: string,
  itemId: string,
  actionIndex: number,
  renderedHash: string,
): string {
  return `${sourceId}\0${itemId}\0${actionIndex}\0${renderedHash}`;
}

function actionPath(
  workspace: LoadedWorkspace,
  source: LoadedWorkspaceSource,
  root?: string,
): string {
  const planHash = shortHash(stableJson({
    id: source.id,
    uses: source.packageName,
    config: source.source.config,
    actions: source.actions.map((action) => ({ uses: action.packageName, with: action.config })),
  }));
  return join(
    workspaceStoreRoot(workspace, root),
    `actions-${sourceSlug(source)}-${planHash}.jsonl`,
  );
}

function sourceSlug(source: LoadedWorkspaceSource): string {
  const identity = source.itemBucketIdentity;
  return `${slugify(source.packageName)}-${slugify(identity).slice(0, 48)}-${shortHash(identity)}`;
}

function sourceIdSlug(sourceId: string): string {
  return `${slugify(sourceId).slice(0, 48)}-${shortHash(sourceId)}`;
}

async function acquireNativeLock(fd: number): Promise<() => void> {
  const { dlopen, FFIType, ptr } = await import("bun:ffi");
  if (process.platform === "win32") {
    const runtime = dlopen("msvcrt.dll", {
      _get_osfhandle: { args: [FFIType.i32], returns: FFIType.i64 },
    });
    const kernel = dlopen("kernel32.dll", {
      LockFileEx: {
        args: [FFIType.i64, FFIType.u32, FFIType.u32, FFIType.u32, FFIType.u32, FFIType.ptr],
        returns: FFIType.bool,
      },
      UnlockFileEx: {
        args: [FFIType.i64, FFIType.u32, FFIType.u32, FFIType.u32, FFIType.ptr],
        returns: FFIType.bool,
      },
    });
    const handle = runtime.symbols._get_osfhandle(fd);
    const overlapped = new Uint8Array(32);
    if (!kernel.symbols.LockFileEx(handle, 3, 0, 1, 0, ptr(overlapped))) {
      runtime.close();
      kernel.close();
      throw new Error("LockFileEx failed");
    }
    return () => {
      kernel.symbols.UnlockFileEx(handle, 0, 1, 0, ptr(overlapped));
      runtime.close();
      kernel.close();
    };
  }
  const library = dlopen(process.platform === "darwin" ? "libSystem.B.dylib" : "libc.so.6", {
    flock: { args: [FFIType.i32, FFIType.i32], returns: FFIType.i32 },
  });
  if (library.symbols.flock(fd, 6) !== 0) {
    library.close();
    throw new Error("flock failed");
  }
  return () => {
    library.symbols.flock(fd, 8);
    library.close();
  };
}

async function readJsonLines<T>(path: string): Promise<T[]> {
  let text: string;
  try {
    text = await readFile(path, "utf8");
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return [];
    throw error;
  }
  const lines = text.endsWith("\n") ? text.trimEnd().split("\n") : text.split("\n").slice(0, -1);
  return lines.filter(Boolean).map((line) => JSON.parse(line) as T);
}

function stableJson(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(stableJson).join(",")}]`;
  if (value !== null && typeof value === "object") {
    return `{${Object.entries(value)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, item]) => `${JSON.stringify(key)}:${stableJson(item)}`)
      .join(",")}}`;
  }
  return JSON.stringify(value) ?? "null";
}

function hash(value: string): string {
  return createHash("sha256").update(value).digest("hex");
}

function shortHash(value: string): string {
  return hash(value).slice(0, 12);
}

function throwIfCancelled(cancellation: AbortSignal): void {
  if (cancellation.aborted) throw cancellation.reason ?? new Error("cancelled");
}

function slugify(value: string): string {
  return value.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "source";
}

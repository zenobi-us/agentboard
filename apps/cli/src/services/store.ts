import { createHash } from "node:crypto";
import { appendFile, mkdir, open, readFile, rename, writeFile } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";

import type { ActionResult, Item, PipelineState } from "@clankpipe/core/config";
import { pluginFor } from "@clankpipe/core/config";
import { renderActionInputs } from "./template.ts";

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

export interface PipelineExecution {
  readonly workspace_id: string;
  readonly source_id: string;
  readonly item_id: string;
  readonly action_plan_hash: string;
  readonly state: PipelineState;
  readonly item: Item;
  readonly ts: string;
  readonly action_index?: number;
  readonly message?: string;
}

export function workspaceStoreRoot(workspace: LoadedWorkspace, root?: string): string {
  if (root) return join(root, workspace.id);
  const dataHome = process.env["XDG_DATA_HOME"] ?? join(homedir(), ".local", "share");
  return join(dataHome, "clankpipe", workspace.id);
}

function workspaceStoreRoots(workspace: LoadedWorkspace, root?: string): string[] {
  if (root) return [join(root, workspace.id)];
  const dataHome = process.env["XDG_DATA_HOME"] ?? join(homedir(), ".local", "share");
  return [join(dataHome, "agentboard", workspace.id), workspaceStoreRoot(workspace)];
}

async function readStoreLines<T>(
  workspace: LoadedWorkspace,
  path: (root: string) => string,
  root?: string,
): Promise<T[]> {
  return (await Promise.all(workspaceStoreRoots(workspace, root).map((value) => readJsonLines<T>(path(value))))).flat();
}

function storeSourceVariants(source: LoadedWorkspaceSource): LoadedWorkspaceSource[] {
  if (!source.packageName.startsWith("@clankpipe/") && !source.packageName.startsWith("clankpipe-source-")) return [source];
  const packageName = source.packageName
    .replace(/^@clankpipe\//, "@agentboard/")
    .replace(/^clankpipe-source-/, "agentboard-source-");
  const actions = source.actions.map((action) => ({
    ...action,
    packageName: action.packageName
      .replace(/^@clankpipe\//, "@agentboard/")
      .replace(/^clankpipe-action-/, "agentboard-action-"),
  }));
  return [{ ...source, packageName, actions }, source];
}

async function readSourceStoreLines<T>(
  workspace: LoadedWorkspace,
  source: LoadedWorkspaceSource,
  path: (storeRoot: string, source: LoadedWorkspaceSource) => string,
  root?: string,
): Promise<T[]> {
  const variants = storeSourceVariants(source).reverse();
  return (await Promise.all(variants.map((variant) => readStoreLines<T>(workspace, (storeRoot) => path(storeRoot, variant), root)))).flat();
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
  const snapshotKey = sourceSnapshotKey(source);
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
  if (cancellation.aborted) return;
}

export async function successfulActionKeys(
  workspace: LoadedWorkspace,
  source: LoadedWorkspaceSource,
  root?: string,
): Promise<Set<string>> {
  const attempts = await readSourceStoreLines<ActionAttempt>(workspace, source, (storeRoot, variant) => join(storeRoot, `actions-${sourceSlug(variant)}-${actionPlanHash(variant)}.jsonl`), root);
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

export async function appendPipelineState(
  workspace: LoadedWorkspace,
  source: LoadedWorkspaceSource,
  execution: PipelineExecution,
  root?: string,
): Promise<void> {
  await mkdir(workspaceStoreRoot(workspace, root), { recursive: true });
  await appendFile(pipelinePath(workspace, source, root), `${JSON.stringify(execution)}\n`);
}

export async function readPipelineExecutions(
  workspace: LoadedWorkspace,
  source: LoadedWorkspaceSource,
  root?: string,
): Promise<PipelineExecution[]> {
  const records = await readSourceStoreLines<PipelineExecution>(workspace, source, (storeRoot, variant) => join(storeRoot, `pipelines-${sourceSlug(variant)}-${actionPlanHash(variant)}.jsonl`), root);
  const latest = new Map<string, PipelineExecution>();
  for (const record of records) {
    latest.set(`${record.source_id}\0${record.item_id}\0${record.action_plan_hash}`, record);
  }
  return [...latest.values()];
}

export async function markStalePipelineExecutions(
  workspace: LoadedWorkspace,
  source: LoadedWorkspaceSource,
  root?: string,
): Promise<void> {
  for (const execution of await readPipelineExecutions(workspace, source, root)) {
    if (execution.state === "claimed" || execution.state === "running") {
      await appendPipelineState(workspace, source, {
        ...execution,
        state: "stale",
        ts: new Date().toISOString(),
        message: "previous Run did not finish",
      }, root);
    }
  }
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

export interface StoredSnapshotItem {
  readonly item: Item;
  readonly sourceSlug: string;
  readonly actionState: "pending" | "succeeded" | "failed";
}

export interface StoredSourceSnapshot {
  readonly sourceId: string;
  readonly sourceSlug: string;
  readonly state: "missing" | "ready";
  readonly items: readonly StoredSnapshotItem[];
  readonly pipeline?: readonly PipelineExecution[];
  readonly collectionStatus?: SourceCollectionStatus;
}

export interface StoredItemView {
  readonly item: Item;
  readonly sourceSlug: string;
  readonly actionState: "pending" | "succeeded" | "failed";
  readonly actions: readonly ActionAttempt[];
}

export async function readStoreViews(workspace: LoadedWorkspace, root?: string): Promise<StoredSourceSnapshot[]> {
  const attempts = await readStoredActions(workspace, root);
  const snapshots: StoredSourceSnapshot[] = [];
  for (const source of workspace.sources) {
    let selected: LoadedWorkspaceSource = source;
    let boundary: { snapshot_key: string; snapshot_id: string } | undefined;
    let records: Record<string, unknown>[] = [];
    for (const variant of storeSourceVariants(source).reverse()) {
      const slug = sourceSlug(variant);
      const boundaries = await readStoreLines<{ snapshot_key: string; snapshot_id: string }>(workspace, (storeRoot) => join(storeRoot, `items-${slug}.snapshots`), root);
      const candidate = [...boundaries].reverse().find((value) => value.snapshot_key === sourceSnapshotKey(variant));
      if (!candidate) continue;
      selected = variant;
      boundary = candidate;
      records = await readStoreLines<Record<string, unknown>>(workspace, (storeRoot) => join(storeRoot, `items-${slug}.jsonl`), root);
      break;
    }
    const slug = sourceSlug(selected);
    const current = boundary
      ? records.filter((value) => value["_agentboard_snapshot_key"] === boundary.snapshot_key && value["_agentboard_snapshot_id"] === boundary.snapshot_id)
      : [];
    const uniqueItems = [...new Map(current.map((value) => [String(value["id"]), value])).values()];
    const pipeline = await readPipelineExecutions(workspace, selected, root);
    const items = uniqueItems.map((value) => {
      const item = stripSnapshotFields(value) as unknown as Item;
      return { item, sourceSlug: slug, actionState: actionPlanState(workspace, selected, item, attempts) };
    }).sort((a, b) => a.item.reference_id.localeCompare(b.item.reference_id) || a.item.id.localeCompare(b.item.id));
    snapshots.push({
      sourceId: source.id,
      sourceSlug: slug,
      state: boundary ? "ready" : "missing",
      items,
      pipeline,
      collectionStatus: await readSourceCollectionStatus(workspace, source.id, root),
    });
  }
  return snapshots;
}

export async function readStoredItems(workspace: LoadedWorkspace, root?: string): Promise<StoredItemView[]> {
  const attempts = await readStoredActions(workspace, root);
  const result: StoredItemView[] = [];
  for (const snapshot of await readStoreViews(workspace, root)) {
    const source = workspace.sources.find((value) => value.id === snapshot.sourceId)!;
    for (const value of snapshot.items) {
      result.push({ ...value, actions: attempts.filter((attempt) => attempt.source_id === source.id && attempt.item_id === value.item.id) });
    }
  }
  return result.sort((a, b) => a.item.reference_id.localeCompare(b.item.reference_id) || a.item.id.localeCompare(b.item.id));
}

async function readStoredActions(workspace: LoadedWorkspace, root?: string): Promise<ActionAttempt[]> {
  const attempts: ActionAttempt[] = [];
  for (const source of workspace.sources) {
    for (const record of await readSourceStoreLines<Record<string, unknown>>(workspace, source, (storeRoot, variant) => join(storeRoot, `actions-${sourceSlug(variant)}-${actionPlanHash(variant)}.jsonl`), root)) {
      const normalized: Record<string, unknown> = { ...record };
      if (normalized["outcome"] === undefined && normalized["success"] !== undefined) {
        normalized["outcome"] = normalized["success"] ? "success" : "failure";
      }
      delete normalized["success"];
      attempts.push(normalized as unknown as ActionAttempt);
    }
  }
  return attempts;
}

function actionPlanState(
  workspace: LoadedWorkspace,
  source: LoadedWorkspaceSource,
  item: Item,
  attempts: readonly ActionAttempt[],
): "pending" | "succeeded" | "failed" {
  if (source.actions.length === 0) return "succeeded";
  const named: Record<string, { inputs: unknown }> = {};
  let pending = false;
  for (const [index, action] of source.actions.entries()) {
    try {
      const inputs = renderActionInputs(action.config, {
        workspace: { id: workspace.id, path: workspace.path },
        source: { id: source.id, source: { uses: source.packageName, ...(source.source.config && typeof source.source.config === "object" ? source.source.config : { value: source.source.config }) }, actions: source.actions.map((value) => ({ id: value.id, uses: value.packageName, with: value.config })) },
        item,
        action: { index, uses: action.packageName },
        actions: named,
      }, { pathInputs: pluginFor(action).pathInputs ?? [] });
      if (action.id) named[action.id] = { inputs };
      const latest = [...attempts].reverse().find((attempt) => attempt.source_id === source.id && attempt.item_id === item.id && attempt.source_action_index === index && attempt.rendered_action_hash === renderedActionHash(action.packageName, inputs));
      if (latest?.outcome === "failure") return "failed";
      if (latest?.outcome !== "success") pending = true;
    } catch {
      return "failed";
    }
  }
  return pending ? "pending" : "succeeded";
}

function stripSnapshotFields(value: Record<string, unknown>): Record<string, unknown> {
  const item = { ...value };
  delete item["_agentboard_snapshot_key"];
  delete item["_agentboard_snapshot_id"];
  return item;
}

function actionPath(
  workspace: LoadedWorkspace,
  source: LoadedWorkspaceSource,
  root?: string,
): string {
  const planHash = actionPlanHash(source);
  return join(
    workspaceStoreRoot(workspace, root),
    `actions-${sourceSlug(source)}-${planHash}.jsonl`,
  );
}

function pipelinePath(
  workspace: LoadedWorkspace,
  source: LoadedWorkspaceSource,
  root?: string,
): string {
  return join(
    workspaceStoreRoot(workspace, root),
    `pipelines-${sourceSlug(source)}-${actionPlanHash(source)}.jsonl`,
  );
}

export function sourceSlug(source: LoadedWorkspaceSource): string {
  const kind = sourceKind(source);
  const identity = source.itemBucketIdentity;
  const packageHash = `${source.packageName}\0${identity}`;
  return `${slugify(kind)}-${slugify(identity)}-${shortHash(packageHash)}`;
}

function sourceKind(source: LoadedWorkspaceSource): string {
  return source.packageName.split("/").at(-1)?.replace(/^source-/, "") ?? source.packageName;
}

function sourceSnapshotKey(source: LoadedWorkspaceSource): string {
  const config = JSON.stringify(source.source.config ?? {});
  const packageIdentity = source.packageName.startsWith("@clankpipe/") || source.packageName.startsWith("clankpipe-source-")
    ? sourceKind(source)
    : source.packageName;
  return defaultHasher([source.id, packageIdentity, config]);
}

export function actionPlanHash(source: LoadedWorkspaceSource): string {
  return shortHash(JSON.stringify({
    id: source.id,
    source: sourceConfig(source),
    actions: source.actions.map((action) => ({ uses: action.packageName, with: action.config })),
  }));
}

function sourceConfig(source: LoadedWorkspaceSource): Record<string, unknown> {
  const config = source.source.config;
  return config !== null && typeof config === "object" && !Array.isArray(config)
    ? { kind: sourceKind(source), ...(config as Record<string, unknown>) }
    : { kind: sourceKind(source), value: config };
}

function defaultHasher(values: readonly string[]): string {
  const encoded = values.map((value) => new TextEncoder().encode(value));
  const bytes = new Uint8Array(encoded.reduce((size, value) => size + value.length + 1, 0));
  let byteOffset = 0;
  for (const value of encoded) {
    bytes.set(value, byteOffset);
    byteOffset += value.length;
    bytes[byteOffset] = 0xff;
    byteOffset += 1;
  }
  const mask = 0xffffffffffffffffn;
  let v0 = 0x736f6d6570736575n;
  let v1 = 0x646f72616e646f6dn;
  let v2 = 0x6c7967656e657261n;
  let v3 = 0x7465646279746573n;
  const rotateLeft = (value: bigint, bits: bigint) => ((value << bits) | (value >> (64n - bits))) & mask;
  const round = () => {
    v0 = (v0 + v1) & mask;
    v1 = rotateLeft(v1, 13n) ^ v0;
    v0 = rotateLeft(v0, 32n);
    v2 = (v2 + v3) & mask;
    v3 = rotateLeft(v3, 16n) ^ v2;
    v0 = (v0 + v3) & mask;
    v3 = rotateLeft(v3, 21n) ^ v0;
    v2 = (v2 + v1) & mask;
    v1 = rotateLeft(v1, 17n) ^ v2;
    v2 = rotateLeft(v2, 32n);
  };
  let offset = 0;
  while (offset + 8 <= bytes.length) {
    let message = 0n;
    for (let index = 0; index < 8; index += 1) message |= BigInt(bytes[offset + index]!) << BigInt(index * 8);
    offset += 8;
    v3 ^= message;
    round();
    v0 ^= message;
  }
  let tail = BigInt(bytes.length) << 56n;
  for (let index = 0; offset + index < bytes.length; index += 1) tail |= BigInt(bytes[offset + index]!) << BigInt(index * 8);
  v3 ^= tail;
  round();
  v0 ^= tail;
  v2 ^= 0xffn;
  round();
  round();
  round();
  return (v0 ^ v1 ^ v2 ^ v3).toString(16).padStart(16, "0");
}

function sourceIdSlug(sourceId: string): string {
  return slugify(sourceId).slice(0, 48);
}

async function readSourceCollectionStatus(
  workspace: LoadedWorkspace,
  sourceId: string,
  root?: string,
): Promise<SourceCollectionStatus | undefined> {
  let status: SourceCollectionStatus | undefined;
  for (const storeRoot of workspaceStoreRoots(workspace, root).reverse()) {
    try {
      status = JSON.parse(await readFile(join(storeRoot, "sources", sourceIdSlug(sourceId), "collection-status.json"), "utf8")) as SourceCollectionStatus;
      break;
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
    }
  }
  if (!status) return undefined;
  if (status.state !== "collecting" || await workspaceLockIsHeld(workspace, root)) return status;
  return { ...status, state: "cancelled" };
}

async function workspaceLockIsHeld(workspace: LoadedWorkspace, root?: string): Promise<boolean> {
  try {
    const unlock = await acquireWorkspaceLock(workspace, root);
    await unlock();
    return false;
  } catch {
    return true;
  }
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

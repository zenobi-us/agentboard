import { createHash } from "node:crypto";
import { existsSync, readFileSync, realpathSync } from "node:fs";
import { basename, extname, resolve } from "node:path";
import Type, { type TSchema } from "typebox";
import {
  action,
  copyPluginReference,
  pluginFor,
  source,
  strictPluginSchema,
  type ResolvedAction,
  type ResolvedSource,
  type WorkspaceConfig,
} from "@agentboard/core/config";

import { fileURLToPath, pathToFileURL } from "node:url";

import { prepareActionRuntime } from "../actions.ts";
import {
  createSourceRuntime,
  type LoadedSourceRuntime,
} from "../sources.ts";
import { validateActionInputs } from "../template.ts";
import {
  discoverPluginPackages,
  loadAllPlugins,
  loadSelectedPlugins,
  registerPlugins,
  type PluginRegistry,
} from "../plugins.ts";

export interface WorkspaceSchemas {
  readonly source: TSchema;
  readonly action: TSchema;
  readonly workspace: TSchema;
}

export interface LoadedWorkspaceSource {
  readonly id: string;
  readonly packageName: string;
  readonly itemBucketIdentity: string;
  readonly source: ResolvedSource<TSchema>;
  readonly actions: readonly (ResolvedAction<TSchema> & {
    readonly packageName: string;
    readonly preparedRuntime: ReturnType<typeof prepareActionRuntime>;
  })[];
}

export interface LoadedWorkspace {
  readonly path: string;
  readonly id: string;
  readonly registry: PluginRegistry;
  readonly sources: readonly LoadedSourceRuntime[];
}

export function resolveWorkspaceConfigPath(configPath = ".agentboard.toml"): string {
  const path = resolve(configPath);
  if (path.endsWith(".agentboard.toml")) {
    for (const name of ["agentboard.config.ts", "agentboard.config.js"]) {
      const executable = resolve(path, "..", name);
      if (existsSync(executable)) return executable;
    }
  }
  return path;
}

export async function loadWorkspacePlugins(
  configPath: string,
  packageNames: readonly string[],
  globalRoot?: string,
): Promise<PluginRegistry> {
  const packages = discoverPluginPackages(configPath, globalRoot);
  return registerPlugins(await loadSelectedPlugins(packageNames, packages));
}

export async function loadAllWorkspacePlugins(
  configPath: string,
  globalRoot?: string,
): Promise<PluginRegistry> {
  const packages = discoverPluginPackages(configPath, globalRoot);
  return registerPlugins(await loadAllPlugins(packages));
}

export async function loadExecutableWorkspace(
  configPath: string,
  configuration?: unknown,
  cancellation: AbortSignal = new AbortController().signal,
): Promise<LoadedWorkspace> {
  const path = resolve(configPath);
  let data = configuration;
  if (data === undefined) {
    try {
      const module = await import(pathToFileURL(path).href);
      data = module.default ?? module;
    } catch (error) {
      throw new Error(`Executable Workspace configuration failed for ${path}: ${String(error)}`);
    }
  }
  validateExecutableWorkspace(data, path);
  validateUniqueSourceIds(data.sources, path, "Executable Workspace configuration");
  const sources = data.sources.map((record) => {
    const sourcePlugin = pluginFor(record.source);
    validateActionIds(record.actions ?? [], path, record.id);
    const actions = (record.actions ?? []).map((configured) => {
      validateActionInputs(configured.config);
      const plugin = pluginFor(configured);
      plugin.validate!(configured.config);
      const preparedRuntime = prepareActionRuntime(configured, {
        workspaceId: workspaceId(path),
        sourceId: record.id,
        cancellation,
      });
      const loaded = {
        ...configured,
        packageName: packageNameForPlugin(pluginFor(configured)),
        preparedRuntime,
      };
      copyPluginReference(configured, loaded);
      return loaded;
    });
    return {
      ...record,
      packageName: packageNameForPlugin(sourcePlugin),
      itemBucketIdentity: sourcePlugin.itemBucketIdentity!(record.source.config),
      actions,
    };
  });
  return {
    path,
    id: workspaceId(path),
    registry: { sources: new Map(), actions: new Map() },
    sources: await buildSourceRuntimes(sources, path, cancellation),
  };
}

export async function loadDataWorkspace(
  configPath: string,
  globalRoot?: string,
  cancellation: AbortSignal = new AbortController().signal,
): Promise<LoadedWorkspace> {
  const path = resolve(configPath);
  const data = parseDataWorkspace(path);
  validateUniqueSourceIds(data.sources, path, "Workspace data validation");
  const packageNames = new Set<string>();
  for (const item of data.sources ?? []) {
    packageNames.add(readPackageName(item.source, `source ${item.id}`));
    for (const configured of item.actions ?? []) {
      packageNames.add(readPackageName(configured, `action in source ${item.id}`));
    }
  }
  const registry = await loadWorkspacePlugins(path, [...packageNames], globalRoot);
  const sources = (data.sources ?? []).map((item) => {
    const sourceName = readPackageName(item.source, `source ${item.id}`);
    const sourcePackage = registry.sources.get(sourceName);
    if (!sourcePackage) throw new Error(`Plugin Package "${sourceName}" is an Action`);
    const { uses: _uses, ...sourceConfig } = item.source;
    rejectReservedPluginId(sourceConfig);
    const resolvedSource = source(
      sourcePackage.plugin as never,
      sourceConfig as never,
      path,
    ) as ResolvedSource<TSchema>;
    validateActionIds(item.actions ?? [], path, item.id);
    const actions = (item.actions ?? []).map((configured) => {
      const actionName = readPackageName(configured, `action in source ${item.id}`);
      const actionPackage = registry.actions.get(actionName);
      if (!actionPackage) throw new Error(`Plugin Package "${actionName}" is a Source`);
      if (!("with" in configured)) {
        throw new Error(`action in source ${item.id} must define "with"`);
      }
      const { uses: _actionUses, with: inputs, ...metadata } = configured;
      rejectReservedPluginId(inputs);
      const { id, ...unexpectedFields } = metadata;
      if (Object.keys(unexpectedFields).length > 0) {
        throw new Error(`action in source ${item.id} payload fields must be inside "with"`);
      }
      if (id !== undefined && typeof id !== "string") {
        throw new TypeError("configuration id must be a string");
      }
      validateActionInputs(inputs);
      actionPackage.plugin.validate!(inputs);
      const resolved = action(
        actionPackage.plugin as never,
        inputs as never,
        path,
      ) as ResolvedAction<TSchema>;
      const preparedRuntime = prepareActionRuntime(resolved, {
        workspaceId: workspaceId(path),
        sourceId: item.id,
        cancellation,
      });
      const loaded = {
        ...resolved,
        id,
        packageName: actionName,
        preparedRuntime,
      } as ResolvedAction<TSchema> & {
        readonly packageName: string;
        readonly preparedRuntime: ReturnType<typeof prepareActionRuntime>;
      };
      copyPluginReference(resolved, loaded);
      return loaded;
    });
    return {
      id: item.id,
      packageName: sourceName,
      itemBucketIdentity: sourcePackage.plugin.itemBucketIdentity!(resolvedSource.config),
      source: resolvedSource,
      actions,
    };
  });
  return {
    path,
    id: workspaceId(path),
    registry,
    sources: await buildSourceRuntimes(sources, path, cancellation),
  };
}

async function buildSourceRuntimes(
  sources: readonly LoadedWorkspaceSource[],
  path: string,
  cancellation: AbortSignal,
): Promise<LoadedSourceRuntime[]> {
  try {
    return await Promise.all(
      sources.map((source) => createSourceRuntime(source, workspaceId(path), cancellation)),
    );
  } catch (error) {
    throw new Error(`Workspace runtime factory failed for ${path}: ${String(error)}`);
  }
}

function workspaceId(path: string): string {
  const canonical = existsSync(path) ? realpathSync(path) : resolve(path);
  const stem = basename(path).replace(/\.[^.]+$/, "") || "workspace";
  const hash = createHash("sha256").update(canonical).digest("hex").slice(0, 12);
  return `${stem}-${hash}`;
}

function validateActionIds(
  actions: readonly { readonly id?: unknown }[],
  path: string,
  sourceId: string,
): void {
  const ids = new Set<string>();
  for (const action of actions) {
    if (action.id === undefined) continue;
    if (typeof action.id !== "string" || !/^[A-Za-z_]\w*$/.test(action.id)) {
      throw new Error(
        `Workspace configuration failed for ${path}: source ${sourceId} has invalid Action id`,
      );
    }
    if (ids.has(action.id)) {
      throw new Error(
        `Workspace configuration failed for ${path}: source ${sourceId} has duplicate Action id "${action.id}"`,
      );
    }
    ids.add(action.id);
  }
}

function validateUniqueSourceIds(
  sources: readonly { readonly id: string }[],
  path: string,
  label: string,
): void {
  const ids = new Set<string>();
  for (const source of sources) {
    if (ids.has(source.id)) {
      throw new Error(`${label} failed for ${path}: duplicate Source id "${source.id}"`);
    }
    ids.add(source.id);
  }
}

function parseDataWorkspace(path: string): {
  sources: Array<{
    id: string;
    source: Record<string, unknown>;
    actions?: Array<Record<string, unknown>>;
  }>;
} {
  const text = readFileSync(path, "utf8");
  let value: unknown;
  try {
    value = extname(path) === ".json"
      ? JSON.parse(text)
      : extname(path) === ".yaml" || extname(path) === ".yml"
        ? Bun.YAML.parse(text)
        : Bun.TOML.parse(text);
  } catch (error) {
    throw new Error(`Workspace data parse failed for ${path}: ${String(error)}`);
  }
  validateWorkspaceData(value, path);
  return value;
}

function validateExecutableWorkspace(
  value: unknown,
  path: string,
): asserts value is WorkspaceConfig {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`Executable Workspace configuration failed for ${path}: expected an object`);
  }
  const sources = (value as Record<string, unknown>)["sources"];
  if (!Array.isArray(sources)) {
    throw new Error(`Executable Workspace configuration failed for ${path}: "sources" must be an array`);
  }
  for (const [index, item] of sources.entries()) {
    if (!item || typeof item !== "object" || Array.isArray(item)) {
      throw new Error(`Executable Workspace configuration failed for ${path}: source ${index} must be an object`);
    }
    const record = item as Partial<WorkspaceConfig["sources"][number]> & { actions?: unknown };
    if (typeof record.id !== "string" || record.id.length === 0) {
      throw new Error(`Executable Workspace configuration failed for ${path}: source ${index} must define a string "id"`);
    }
    if (record.actions !== undefined && !Array.isArray(record.actions)) {
      throw new Error(`Executable Workspace configuration failed for ${path}: source ${record.id} actions must be an array`);
    }
    try {
      const sourcePlugin = pluginFor(record.source as object);
      if (sourcePlugin.kind !== "source") throw new TypeError("configuration node is not a Source");
      for (const configured of record.actions ?? []) {
        const actionPlugin = pluginFor(configured as object);
        if (actionPlugin.kind !== "action") throw new TypeError("configuration node is not an Action");
      }
    } catch (error) {
      throw new Error(`Executable Workspace configuration failed for ${path}: source ${record.id} is invalid: ${String(error)}`);
    }
  }
}

function packageNameForPlugin(plugin: ReturnType<typeof pluginFor>): string {
  if (plugin.meta.packageName) return plugin.meta.packageName;
  if (!plugin.meta.url.startsWith("file:")) return plugin.meta.url;
  let directory = resolve(fileURLToPath(plugin.meta.url), "..");
  while (true) {
    const manifestPath = resolve(directory, "package.json");
    if (existsSync(manifestPath)) {
      const manifest = JSON.parse(readFileSync(manifestPath, "utf8")) as {
        name?: unknown;
        keywords?: unknown;
      };
      if (
        typeof manifest.name === "string" &&
        Array.isArray(manifest.keywords) &&
        manifest.keywords.includes("agentboard-package")
      ) return manifest.name;
    }
    const parent = resolve(directory, "..");
    if (parent === directory) break;
    directory = parent;
  }
  return plugin.meta.url;
}

function validateWorkspaceData(value: unknown, path: string): asserts value is {
  sources: Array<{
    id: string;
    source: Record<string, unknown>;
    actions?: Array<Record<string, unknown>>;
  }>;
} {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`Workspace data validation failed for ${path}: expected an object`);
  }
  const sources = (value as Record<string, unknown>)["sources"];
  if (!Array.isArray(sources)) {
    throw new Error(`Workspace data validation failed for ${path}: "sources" must be an array`);
  }
  for (const [index, item] of sources.entries()) {
    if (!item || typeof item !== "object" || Array.isArray(item)) {
      throw new Error(`Workspace data validation failed for ${path}: source ${index} must be an object`);
    }
    const record = item as Record<string, unknown>;
    const id = record["id"];
    const source = record["source"];
    const actions = record["actions"];
    if (typeof id !== "string" || id.length === 0) {
      throw new Error(`Workspace data validation failed for ${path}: source ${index} must define a string "id"`);
    }
    if (!source || typeof source !== "object" || Array.isArray(source)) {
      throw new Error(`Workspace data validation failed for ${path}: source ${id} must define an object "source"`);
    }
    if (actions !== undefined && (!Array.isArray(actions) || actions.some((action) => !action || typeof action !== "object" || Array.isArray(action)))) {
      throw new Error(`Workspace data validation failed for ${path}: source ${id} actions must be an array of objects`);
    }
  }
}


function readPackageName(value: Record<string, unknown>, location: string): string {
  const name = value["uses"];
  if (typeof name !== "string" || name.length === 0) {
    throw new Error(`${location} must define a Plugin Package in "uses"`);
  }
  return name;
}

function rejectReservedPluginId(value: unknown): void {
  if (value !== null && typeof value === "object" && Object.hasOwn(value, "id")) {
    throw new Error('Plugin payload must not define reserved field "id"');
  }
}

export function createWorkspaceSchemas(registry: PluginRegistry): WorkspaceSchemas {
  const source = union(
    [...registry.sources.values()].map(({ package: item, plugin }) =>
      flatSourceSchema(item.name, plugin.schema),
    ),
  );
  const action = union(
    [...registry.actions.values()].map(({ package: item, plugin }) =>
      Type.Object(
        {
          id: Type.Optional(Type.String()),
          uses: Type.Literal(item.name),
          with: reservePluginId(strictPluginSchema(plugin.schema)),
        },
        { additionalProperties: false },
      ),
    ),
  );
  const workspaceSource = Type.Object({
    id: Type.String(),
    source,
    actions: Type.Optional(Type.Array(action, { default: [] })),
  });

  return {
    source,
    action,
    workspace: Type.Object({ sources: Type.Array(workspaceSource) }),
  };
}

function flatSourceSchema(name: string, schema: TSchema): TSchema {
  if (
    typeof schema === "object" &&
    schema !== null &&
    "type" in schema &&
    schema.type === "object"
  ) {
    const objectSchema = schema as TSchema & {
      properties?: Record<string, TSchema>;
      required?: readonly string[];
    };
    const properties = objectSchema.properties ?? {};
    const required = objectSchema.required ?? [];
    return {
      ...schema,
      additionalProperties: "additionalProperties" in schema
        ? schema.additionalProperties
        : false,
      properties: {
        ...properties,
        id: Type.Optional(Type.Never()),
        uses: Type.Literal(name),
      },
      required: ["uses", ...required.filter((item) => item !== "uses")],
    };
  }
  return Type.Intersect([Type.Object({ uses: Type.Literal(name) }), schema]);
}

function reservePluginId(schema: TSchema): TSchema {
  if (
    typeof schema === "object" &&
    schema !== null &&
    "type" in schema &&
    schema.type === "object"
  ) {
    const properties = "properties" in schema
      ? schema.properties as Record<string, TSchema>
      : {};
    return {
      ...schema,
      properties: { ...properties, id: Type.Optional(Type.Never()) },
    };
  }
  return schema;
}

function union(schemas: TSchema[]): TSchema {
  if (schemas.length === 0) return Type.Never();
  if (schemas.length === 1) return schemas[0]!;
  return Type.Union(schemas);
}

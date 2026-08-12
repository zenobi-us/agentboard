import { existsSync, readFileSync } from "node:fs";
import { extname, resolve } from "node:path";
import Type, { type TSchema } from "typebox";
import {
  action,
  pluginFor,
  source,
  type ResolvedAction,
  type ResolvedSource,
  type WorkspaceConfig,
} from "@agentboard/core/config";

import { pathToFileURL } from "node:url";

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
  readonly source: ResolvedSource<TSchema>;
  readonly actions: readonly (ResolvedAction<TSchema> & { readonly packageName: string })[];
}

export interface LoadedWorkspace {
  readonly path: string;
  readonly registry: PluginRegistry;
  readonly sources: readonly LoadedWorkspaceSource[];
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
): Promise<LoadedWorkspace> {
  const path = resolve(configPath);
  let data: unknown;
  try {
    const module = await import(pathToFileURL(path).href);
    data = module.default ?? module;
  } catch (error) {
    throw new Error(`Executable Workspace configuration failed for ${path}: ${String(error)}`);
  }
  validateExecutableWorkspace(data, path);
  return {
    path,
    registry: { sources: new Map(), actions: new Map() },
    sources: data.sources.map((record) => {
      const sourcePlugin = pluginFor(record.source);
      return {
        ...record,
        packageName: sourcePlugin.meta.packageName ?? sourcePlugin.meta.url,
        actions: (record.actions ?? []).map((configured) => ({
          ...configured,
          packageName: packageNameFor(configured),
        })),
      };
    }),
  };
}

export async function loadDataWorkspace(
  configPath: string,
  globalRoot?: string,
): Promise<LoadedWorkspace> {
  const path = resolve(configPath);
  const data = parseDataWorkspace(path);
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
    const resolvedSource = source(sourcePackage.plugin as never, {
      ...sourceConfig,
      id: item.id,
    } as never, path) as ResolvedSource<TSchema>;
    const actions = (item.actions ?? []).map((configured) => {
      const actionName = readPackageName(configured, `action in source ${item.id}`);
      const actionPackage = registry.actions.get(actionName);
      if (!actionPackage) throw new Error(`Plugin Package "${actionName}" is a Source`);
      const { uses: _actionUses, with: inputs, ...metadata } = configured;
      return {
        ...action(
          actionPackage.plugin as never,
          { ...metadata, ...(inputs ?? {}) } as never,
          path,
        ),
        packageName: actionName,
      } as ResolvedAction<TSchema> & { readonly packageName: string };
    });
    return { id: item.id, packageName: sourceName, source: resolvedSource, actions };
  });
  return { path, registry, sources };
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

function packageNameFor(configured: ResolvedAction<TSchema>): string {
  const plugin = pluginFor(configured);
  return plugin.meta.packageName ?? plugin.meta.url;
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

export function createWorkspaceSchemas(registry: PluginRegistry): WorkspaceSchemas {
  const source = union(
    [...registry.sources.values()].map(({ package: item, plugin }) =>
      flatSourceSchema(item.name, plugin.schema),
    ),
  );
  const action = union(
    [...registry.actions.values()].map(({ package: item, plugin }) =>
      Type.Object({ uses: Type.Literal(item.name), with: plugin.schema }),
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
      properties: { ...properties, uses: Type.Literal(name) },
      required: ["uses", ...required.filter((item) => item !== "uses")],
    };
  }
  return Type.Intersect([Type.Object({ uses: Type.Literal(name) }), schema]);
}

function union(schemas: TSchema[]): TSchema {
  if (schemas.length === 0) return Type.Never();
  if (schemas.length === 1) return schemas[0]!;
  return Type.Union(schemas);
}

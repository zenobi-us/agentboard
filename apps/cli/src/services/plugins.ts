import { existsSync, readFileSync, readdirSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

import {
  isPluginDescriptor,
  type Plugin,
  type PluginRole,
} from "@agentboard/core/config";
import type { TSchema } from "typebox";

type AnyPlugin = Plugin<PluginRole, TSchema>;

type PackageManifest = {
  readonly name?: unknown;
  readonly keywords?: unknown;
  readonly main?: unknown;
  readonly exports?: unknown;
};

export interface PluginPackage {
  readonly name: string;
  readonly root: string;
  readonly manifestPath: string;
  readonly manifest: PackageManifest;
}

export interface LoadedPluginPackage {
  readonly package: PluginPackage;
  readonly plugin: AnyPlugin;
}

export interface PluginRegistry {
  readonly sources: ReadonlyMap<string, LoadedPluginPackage>;
  readonly actions: ReadonlyMap<string, LoadedPluginPackage>;
}

export function globalPluginRoot(): string {
  return join(homedir(), ".local", "share", "agentboard", "plugins", "npm");
}

export function findProjectPackageRoot(configPath: string): string {
  let directory = dirname(resolve(configPath));
  while (true) {
    if (existsSync(join(directory, "package.json"))) return directory;
    const parent = dirname(directory);
    if (parent === directory) break;
    directory = parent;
  }
  return dirname(resolve(configPath));
}

export function discoverPluginPackages(
  configPath: string,
  globalRoot = globalPluginRoot(),
): PluginPackage[] {
  const local = discoverNodeModules(join(findProjectPackageRoot(configPath), "node_modules"));
  const global = discoverPackageDirectory(globalRoot);
  const localNames = new Set(local.map((item) => item.name));
  return [...local, ...global.filter((item) => !localNames.has(item.name))];
}

export async function loadAllPlugins(
  packages: readonly PluginPackage[],
): Promise<LoadedPluginPackage[]> {
  const loaded: LoadedPluginPackage[] = [];
  for (const item of packages) loaded.push(await loadPluginPackage(item.name, packages));
  return loaded;
}

export function registerPlugins(
  loaded: readonly LoadedPluginPackage[],
): PluginRegistry {
  const sources = new Map<string, LoadedPluginPackage>();
  const actions = new Map<string, LoadedPluginPackage>();
  const names = new Set<string>();

  for (const item of loaded) {
    if (!names.add(item.package.name)) {
      throw new Error(`duplicate registered Plugin Package "${item.package.name}"`);
    }
    const target = item.plugin.kind === "source" ? sources : actions;
    target.set(item.package.name, item);
  }
  return { sources, actions };
}

export async function loadSelectedPlugins(
  names: readonly string[],
  packages: readonly PluginPackage[],
): Promise<LoadedPluginPackage[]> {
  const loaded: LoadedPluginPackage[] = [];
  const seen = new Set<string>();

  for (const name of names) {
    if (!seen.add(name)) throw new Error(`duplicate selected Plugin Package "${name}"`);
    loaded.push(await loadPluginPackage(name, packages));
  }
  return loaded;
}

export async function loadPluginPackage(
  name: string,
  packages: readonly PluginPackage[],
): Promise<LoadedPluginPackage> {
  const item = packages.find((candidate) => candidate.name === name);
  if (!item) {
    throw new Error(
      `Plugin Package "${name}" is not installed; install it with \`bun add ${name}\``,
    );
  }
  return await importPlugin(item);
}

function discoverNodeModules(root: string): PluginPackage[] {
  if (!existsSync(root)) return [];
  const found: PluginPackage[] = [];
  scanNodeModules(root, found);
  return found;
}

function scanNodeModules(directory: string, found: PluginPackage[]): void {
  for (const entry of entries(directory)) {
    if (entry.name === ".bin") continue;
    const packageRoot = join(directory, entry.name);
    if (entry.name.startsWith("@")) {
      for (const scoped of entries(packageRoot)) {
        inspectPackage(join(packageRoot, scoped.name), found);
      }
      continue;
    }
    inspectPackage(packageRoot, found);
  }
}

function inspectPackage(root: string, found: PluginPackage[]): void {
  const manifestPath = join(root, "package.json");
  if (!existsSync(manifestPath)) return;
  const manifest = readManifest(manifestPath);
  if (!hasPluginKeyword(manifest.keywords)) return;
  if (typeof manifest.name !== "string" || manifest.name.length === 0) {
    throw new Error(`Plugin Package at ${manifestPath} must define package.json name`);
  }
  if (found.some((item) => item.name === manifest.name)) {
    throw new Error(`duplicate Plugin Package identity "${manifest.name}"`);
  }
  found.push({
    name: manifest.name,
    root: resolve(root),
    manifestPath,
    manifest,
  });
  const nested = join(root, "node_modules");
  if (existsSync(nested)) scanNodeModules(nested, found);
}

function discoverPackageDirectory(root: string): PluginPackage[] {
  if (!existsSync(root)) return [];
  const found: PluginPackage[] = [];
  for (const entry of entries(root)) {
    if (entry.name === "node_modules" || entry.name === ".bin") continue;
    const packageRoot = join(root, entry.name);
    if (entry.name.startsWith("@")) {
      for (const scoped of entries(packageRoot)) {
        inspectPackage(join(packageRoot, scoped.name), found);
      }
    } else {
      inspectPackage(packageRoot, found);
    }
  }
  const nested = join(root, "node_modules");
  if (existsSync(nested)) scanNodeModules(nested, found);
  return found;
}

function entries(directory: string): { name: string }[] {
  if (!existsSync(directory)) return [];
  return readdirSync(directory, { withFileTypes: true })
    .filter((entry) => entry.isDirectory() || entry.isSymbolicLink())
    .sort((left, right) => left.name.localeCompare(right.name))
    .map((entry) => ({ name: entry.name }));
}

function readManifest(path: string): PackageManifest {
  try {
    return JSON.parse(readFileSync(path, "utf8")) as PackageManifest;
  } catch (error) {
    throw new Error(`read Plugin Package metadata ${path}: ${String(error)}`);
  }
}

function hasPluginKeyword(keywords: unknown): boolean {
  return Array.isArray(keywords) && keywords.includes("agentboard-package");
}

async function importPlugin(item: PluginPackage): Promise<LoadedPluginPackage> {
  const module = await import(pathToFileURL(packageEntry(item)).href);
  const descriptors = [...new Set(Object.values(module).filter(isPluginDescriptor))];
  if (descriptors.length !== 1) {
    throw new Error(
      `Plugin Package "${item.name}" must provide exactly one Plugin Descriptor; found ${descriptors.length}`,
    );
  }
  if (!isPluginDescriptor(module.default)) {
    throw new Error(
      `Plugin Package "${item.name}" must default export one Plugin Descriptor`,
    );
  }
  return adaptExternalPlugin(item, module.default);
}

export function adaptExternalPlugin(
  item: PluginPackage,
  descriptor: unknown,
): LoadedPluginPackage {
  if (!isPluginDescriptor(descriptor)) {
    throw new TypeError(
      `Plugin Package "${item.name}" must export one Plugin Descriptor created by definePlugin`,
    );
  }
  return {
    package: item,
    plugin: {
      ...descriptor,
      meta: { ...descriptor.meta, packageName: item.name },
    },
  };
}

function packageEntry(item: PluginPackage): string {
  const target = exportTarget(item.manifest.exports) ?? item.manifest.main;
  if (typeof target !== "string") return join(item.root, "index.js");
  return resolve(item.root, target.startsWith("./") ? target : `./${target}`);
}

function exportTarget(exportsField: unknown): string | undefined {
  if (typeof exportsField === "string") return exportsField;
  if (!exportsField || typeof exportsField !== "object") return undefined;
  const record = exportsField as Record<string, unknown>;
  if ("." in record) return exportTarget(record["."]);
  for (const key of ["import", "default", "node", "require"]) {
    const target = exportTarget(record[key]);
    if (target) return target;
  }
  return undefined;
}

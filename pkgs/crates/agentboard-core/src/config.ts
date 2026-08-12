import Type from "typebox";
import type { Static, TSchema } from "typebox";
import { Compile } from "typebox/compile";

export const FieldMapSchema = Type.Object({
  id: Type.Optional(Type.Union([Type.String(), Type.Null()])),
  title: Type.Optional(Type.Union([Type.String(), Type.Null()])),
  status: Type.Optional(Type.Union([Type.String(), Type.Null()])),
  url: Type.Optional(Type.Union([Type.String(), Type.Null()])),
});

export type FieldMap = Static<typeof FieldMapSchema>;

export type PluginRole = "source" | "action";
export type PluginKind = PluginRole;

export interface PluginMeta {
  readonly url: string;
  readonly packageName?: string;
}

export interface Item {
  readonly id: string;
  readonly reference_id: string;
  readonly title: string;
  readonly status: string;
  readonly url: string;
  readonly source_id: string;
  readonly source_kind: string;
  readonly raw: unknown;
}

export interface ActionResult {
  readonly outcome: "success" | "failure" | "cancelled";
  readonly stdout: string;
  readonly stderr: string;
  readonly message?: string;
}

export interface SourceRuntimeContext {
  readonly sourceId: string;
}

export interface ActionRuntimeContext {
  readonly workspaceId: string;
  readonly sourceId: string;
  readonly item: Item;
}

export interface ActionRuntimeFactoryContext {
  readonly workspaceId: string;
  readonly sourceId: string;
}

export type HealthCheckContext = SourceRuntimeContext;

export interface SourceRuntime {
  collect(): Promise<readonly Item[]> | readonly Item[];
}

export interface ActionRuntime {
  execute(context: ActionRuntimeContext): Promise<ActionResult> | ActionResult;
}

type RuntimeFor<Role extends PluginRole> = Role extends "source"
  ? SourceRuntime
  : ActionRuntime;

type RuntimeContextFor<Role extends PluginRole> = Role extends "source"
  ? SourceRuntimeContext
  : ActionRuntimeFactoryContext;

export interface Plugin<
  Role extends PluginRole,
  Schema extends TSchema,
  Runtime extends RuntimeFor<Role> = RuntimeFor<Role>,
> {
  readonly kind: Role;
  readonly schema: Schema;
  readonly runtime: (
    config: Static<Schema>,
    context: RuntimeContextFor<Role>,
  ) => Runtime;
  readonly healthCheck: (
    config: Static<Schema>,
    context: HealthCheckContext,
  ) => unknown;
  readonly meta: PluginMeta;
}

type PluginDefinition<
  Role extends PluginRole,
  Schema extends TSchema,
  Runtime extends RuntimeFor<Role>,
> = Omit<Plugin<Role, Schema, Runtime>, "meta">;

export function definePlugin<
  const Role extends PluginRole,
  const Schema extends TSchema,
  Runtime extends RuntimeFor<Role>,
>(
  module: Pick<ImportMeta, "url">,
  definition: PluginDefinition<Role, Schema, Runtime>,
): Plugin<Role, Schema, Runtime> {
  if (definition.kind !== "source" && definition.kind !== "action") {
    throw new TypeError(`invalid plugin role: ${String(definition.kind)}`);
  }

  return {
    ...definition,
    meta: { url: module.url },
  };
}

export interface ResolvedIdentity {
  readonly path: string;
  readonly role: PluginRole;
  readonly position: number;
}

export interface ResolvedConfiguration<
  Role extends PluginRole,
  Schema extends TSchema,
> {
  readonly kind: Role;
  readonly id: string | undefined;
  readonly identity: ResolvedIdentity;
  readonly config: Static<Schema>;
}

export type ResolvedSource<Schema extends TSchema> = ResolvedConfiguration<
  "source",
  Schema
>;

export type ResolvedAction<Schema extends TSchema> = ResolvedConfiguration<
  "action",
  Schema
>;

export interface WorkspaceConfig {
  readonly sources: readonly WorkspaceSourceConfig[];
}

export interface WorkspaceSourceConfig {
  readonly id: string;
  readonly source: ResolvedSource<TSchema>;
  readonly actions?: readonly ResolvedAction<TSchema>[];
}

/** Preserve inferred Source and Action types for executable Workspace authors. */
export function defineConfig<const Config extends WorkspaceConfig>(
  config: Config,
): Config {
  return config;
}

type ConfigWithId<Schema extends TSchema> = Static<Schema> & {
  id?: string;
};

type AnyPlugin = Plugin<PluginRole, TSchema>;

export function isPluginDescriptor(value: unknown): value is AnyPlugin {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Partial<AnyPlugin>;
  return (
    (candidate.kind === "source" || candidate.kind === "action") &&
    typeof candidate.schema === "object" &&
    candidate.schema !== null &&
    typeof candidate.runtime === "function" &&
    typeof candidate.healthCheck === "function" &&
    typeof candidate.meta?.url === "string"
  );
}

const positions = new Map<PluginRole, Map<string, number>>([
  ["source", new Map()],
  ["action", new Map()],
]);
const pluginReferences = new WeakMap<object, AnyPlugin>();

export function pluginFor(
  value: object,
): AnyPlugin {
  const plugin = pluginReferences.get(value);
  if (!plugin) throw new TypeError("configuration node has no Plugin Descriptor");
  return plugin;
}

export function copyPluginReference(from: object, to: object): void {
  pluginReferences.set(to, pluginFor(from));
}

function nextPosition(role: PluginRole, path: string): number {
  const rolePositions = positions.get(role)!;
  const position = rolePositions.get(path) ?? 0;
  rolePositions.set(path, position + 1);
  return position;
}

export function strictPluginSchema(schema: TSchema): TSchema {
  if (
    typeof schema === "object" &&
    schema !== null &&
    "type" in schema &&
    schema.type === "object" &&
    !("additionalProperties" in schema)
  ) {
    return { ...schema, additionalProperties: false };
  }
  return schema;
}

function splitConfig(config: unknown): { id: string | undefined; payload: unknown } {
  if (config === null || typeof config !== "object" || Array.isArray(config)) {
    return { id: undefined, payload: config };
  }

  const { id, ...payload } = config as Record<string, unknown>;
  if (id !== undefined && typeof id !== "string") {
    throw new TypeError("configuration id must be a string");
  }
  return { id, payload };
}

function resolve<
  const Role extends PluginRole,
  const Schema extends TSchema,
  Runtime extends RuntimeFor<Role>,
>(
  expectedRole: Role,
  plugin: Plugin<Role, Schema, Runtime>,
  config: ConfigWithId<Schema>,
  path: string,
): ResolvedConfiguration<Role, Schema> {
  if (plugin.kind !== expectedRole) {
    throw new TypeError(
      `${plugin.kind} plugin cannot create ${expectedRole} configuration`,
    );
  }

  const { id, payload } = splitConfig(config);
  const normalized = Compile(plugin.schema).Default(payload);
  const validated = Compile(strictPluginSchema(plugin.schema)).Parse(
    normalized,
  ) as Static<Schema>;
  const identity = {
    path,
    role: plugin.kind,
    position: nextPosition(plugin.kind, path),
  } as const;
  const resolved = {
    kind: plugin.kind,
    id,
    identity,
    config: validated,
  } as ResolvedConfiguration<Role, Schema>;

  pluginReferences.set(resolved, plugin as AnyPlugin);
  return resolved;
}

export function source<const Schema extends TSchema, Runtime extends SourceRuntime>(
  plugin: Plugin<"source", Schema, Runtime>,
  config: ConfigWithId<Schema>,
  path: string,
): ResolvedSource<Schema> {
  return resolve("source", plugin, config, path);
}

export function action<const Schema extends TSchema, Runtime extends ActionRuntime>(
  plugin: Plugin<"action", Schema, Runtime>,
  config: ConfigWithId<Schema>,
  path: string,
): ResolvedAction<Schema> {
  return resolve("action", plugin, config, path);
}

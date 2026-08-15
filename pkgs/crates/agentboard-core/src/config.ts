import Type from "typebox";
import type { Static, TSchema } from "typebox";
import { Compile } from "typebox/compile";

/** Maps normalized item fields to source payload fields. */
export const FieldMapSchema = Type.Object({
  /** Source payload field for the item identifier. */
  id: Type.Optional(Type.Union([Type.String(), Type.Null()])),
  /** Source payload field for the item title. */
  title: Type.Optional(Type.Union([Type.String(), Type.Null()])),
  /** Source payload field for the item status. */
  status: Type.Optional(Type.Union([Type.String(), Type.Null()])),
  /** Source payload field for the item URL. */
  url: Type.Optional(Type.Union([Type.String(), Type.Null()])),
});

/** Field mapping derived from {@link FieldMapSchema}. */
export type FieldMap = Static<typeof FieldMapSchema>;

/** Role that a plugin performs in an AgentBoard run. */
export type PluginRole = "source" | "action";
/** Alias for {@link PluginRole}. (Preserves the existing public API name.) */
export type PluginKind = PluginRole;

/** Metadata that identifies the module which defined a plugin. */
export interface PluginMeta {
  /** URL of the module that defined the plugin. */
  readonly url: string;
  /** Package name for display or package resolution when one is available. */
  readonly packageName?: string;
}

/** Normalized work item collected from a source plugin. */
export interface Item {
  /** Stable identifier for the normalized item. */
  readonly id: string;
  /** Identifier used by the external source. */
  readonly reference_id: string;
  /** Human-readable item title. */
  readonly title: string;
  /** Current item status from the source. */
  readonly status: string;
  /** URL for the item in its source system. */
  readonly url: string;
  /** Workspace source identifier that collected the item. */
  readonly source_id: string;
  /** Source plugin kind that collected the item. */
  readonly source_kind: string;
  /** Original source payload. (Keeps source-specific data outside the normalized model.) */
  readonly raw: unknown;
}

/** Result returned after an action attempts to process an item. */
export interface ActionResult {
  /** Final action outcome. */
  readonly outcome: "success" | "failure" | "cancelled";
  /** Text written to standard output by the action. */
  readonly stdout: string;
  /** Text written to standard error by the action. */
  readonly stderr: string;
  /** Optional human-readable result detail. */
  readonly message?: string;
}

/** Values supplied when AgentBoard creates a source runtime. */
export interface SourceRuntimeContext {
  /** Workspace identifier for the configured source. */
  readonly sourceId: string;
  /** Signal that requests cancellation of source work. */
  readonly cancellation: AbortSignal;
}

/** Values supplied when an action processes one item. */
export interface ActionRuntimeContext {
  /** Workspace that owns the action run. */
  readonly workspaceId: string;
  /** Configured source that produced the item. */
  readonly sourceId: string;
  /** Item that the action must process. */
  readonly item: Item;
  /** Signal that requests cancellation of action work. */
  readonly cancellation: AbortSignal;
}

/** Values supplied when AgentBoard creates an action runtime factory. */
export interface ActionRuntimeFactoryContext {
  /** Workspace that owns the action configuration. */
  readonly workspaceId: string;
  /** Configured source that owns the action. */
  readonly sourceId: string;
  /** Signal that requests cancellation of runtime setup. */
  readonly cancellation: AbortSignal;
}

/** Values supplied when AgentBoard checks plugin health. */
export interface HealthCheckContext {
  /** Configured source associated with the health check. */
  readonly sourceId: string;
  /** Signal that requests cancellation of the health check. */
  readonly cancellation: AbortSignal;
}

/** Runtime that collects normalized items from one source. */
export interface SourceRuntime {
  /** Collects the items currently available from the source. */
  collect(): Promise<readonly Item[]> | readonly Item[];
}

/** Collected items plus optional source pagination information. */
export interface SourceCollection {
  /** Normalized items returned by the source. */
  readonly items: readonly Item[];
  /** Total number of items available before a source limit is applied. */
  readonly available?: number;
  /** Maximum number of items requested from the source. */
  readonly limit?: number;
}

/** Runtime that executes an action for normalized items. */
export interface ActionRuntime {
  /** Processes one item and returns its action result. */
  execute(context: ActionRuntimeContext): Promise<ActionResult> | ActionResult;
  /** Reports whether an earlier successful result can be reused. */
  cachedSuccessIsValid?(context: ActionRuntimeContext): Promise<boolean> | boolean;
}

/** Prepared action factory that creates runtimes from resolved action inputs. */
export interface PreparedActionRuntime<Runtime extends ActionRuntime = ActionRuntime> {
  /** Creates an action runtime for the supplied inputs. */
  create(inputs: unknown): Runtime;
}

/** Runtime contract selected for a plugin role. */
type RuntimeFor<Role extends PluginRole> = Role extends "source"
  ? SourceRuntime
  : PreparedActionRuntime;

/** Runtime creation context selected for a plugin role. */
type RuntimeContextFor<Role extends PluginRole> = Role extends "source"
  ? SourceRuntimeContext
  : ActionRuntimeFactoryContext;

/** Complete descriptor for a source or action plugin. */
export interface Plugin<
  Role extends PluginRole,
  Schema extends TSchema,
  Runtime extends RuntimeFor<Role> = RuntimeFor<Role>,
> {
  /** Role performed by the plugin. */
  readonly kind: Role;
  /** TypeBox schema used to default and validate plugin configuration. */
  readonly schema: Schema;
  /** Creates the role-specific runtime from validated configuration. */
  readonly runtime: (
    config: Static<Schema>,
    context: RuntimeContextFor<Role>,
  ) => Runtime;
  /** Checks whether the configured plugin can run. */
  readonly healthCheck: (
    config: Static<Schema>,
    context: HealthCheckContext,
  ) => unknown;
  /** Applies action-specific validation after schema validation. */
  readonly validate?: (config: Static<Schema>) => unknown;
  /** Configuration paths that AgentBoard resolves as filesystem inputs. */
  readonly pathInputs?: readonly string[];
  /** Returns the storage bucket identity for items from a source configuration. */
  readonly itemBucketIdentity?: (config: Static<Schema>) => string;
  /** Module metadata added by {@link definePlugin}. */
  readonly meta: PluginMeta;
}

/** Author-supplied plugin fields before AgentBoard adds module metadata. */
type PluginDefinition<
  Role extends PluginRole,
  Schema extends TSchema,
  Runtime extends RuntimeFor<Role>,
> = Omit<Plugin<Role, Schema, Runtime>, "meta" | "itemBucketIdentity" | "validate"> &
  (Role extends "source"
    ? {
        /** Returns the storage bucket identity for source items. */
        readonly itemBucketIdentity: (config: Static<Schema>) => string;
        /** Prevents source plugins from declaring action validation. */
        readonly validate?: never;
      }
    : {
        /** Prevents action plugins from declaring source bucket identity. */
        readonly itemBucketIdentity?: never;
        /** Applies action-specific validation after schema validation. */
        readonly validate: (config: Static<Schema>) => unknown;
      });

/**
 * Defines a plugin and records the defining module URL.
 *
 * Runtime checks keep JavaScript callers from bypassing the role-specific type rules.
 */
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
  if (definition.kind === "source" && typeof definition.itemBucketIdentity !== "function") {
    throw new TypeError("source plugin must define itemBucketIdentity()");
  }
  if (definition.kind === "action" && typeof definition.validate !== "function") {
    throw new TypeError("action plugin must define validate()");
  }

  return {
    ...definition,
    meta: { url: module.url },
  } as Plugin<Role, Schema, Runtime>;
}

/** Identity assigned to a resolved plugin configuration node. */
export interface ResolvedIdentity {
  /** Configuration path that created the node. */
  readonly path: string;
  /** Plugin role of the node. */
  readonly role: PluginRole;
  /** Zero-based occurrence of the role at the configuration path. */
  readonly position: number;
}

/** Plugin configuration after defaulting, validation, and identity assignment. */
export interface ResolvedConfiguration<
  Role extends PluginRole,
  Schema extends TSchema,
> {
  /** Role performed by the configured plugin. */
  readonly kind: Role;
  /** Optional user-defined configuration identifier. */
  readonly id: string | undefined;
  /** Stable location of the configuration node within the current process. */
  readonly identity: ResolvedIdentity;
  /** Defaulted and validated plugin configuration payload. */
  readonly config: Static<Schema>;
}

/** Resolved configuration for a source plugin. */
export type ResolvedSource<Schema extends TSchema> = ResolvedConfiguration<
  "source",
  Schema
>;

/** Resolved configuration for an action plugin. */
export type ResolvedAction<Schema extends TSchema> = ResolvedConfiguration<
  "action",
  Schema
>;

/** Executable configuration for one AgentBoard workspace. */
export interface WorkspaceConfig {
  /** Source configurations and their attached actions. */
  readonly sources: readonly WorkspaceSourceConfig[];
}

/** One configured source and the actions that process its items. */
export interface WorkspaceSourceConfig {
  /** Workspace-local identifier for the source. */
  readonly id: string;
  /** Resolved source plugin configuration. */
  readonly source: ResolvedSource<TSchema>;
  /** Resolved action plugin configurations attached to the source. */
  readonly actions?: readonly ResolvedAction<TSchema>[];
}

/** Preserves inferred source and action types for executable workspace authors. */
export function defineConfig<const Config extends WorkspaceConfig>(
  config: Config,
): Config {
  return config;
}

/** Plugin configuration payload with its optional node identifier. */
type ConfigWithId<Schema extends TSchema> = Static<Schema> & {
  /** Optional user-defined identifier removed before schema validation. */
  id?: string;
};

/** Plugin descriptor with role and schema details erased for shared storage. */
type AnyPlugin = Plugin<PluginRole, TSchema>;

/** Reports whether a value has the required runtime plugin descriptor fields. */
export function isPluginDescriptor(value: unknown): value is AnyPlugin {
  if (!value || typeof value !== "object") return false;
  /** Candidate descriptor inspected without trusting its shape. */
  const candidate = value as Partial<AnyPlugin>;
  return (
    (candidate.kind === "source" || candidate.kind === "action") &&
    typeof candidate.schema === "object" &&
    candidate.schema !== null &&
    typeof candidate.runtime === "function" &&
    typeof candidate.healthCheck === "function" &&
    (candidate.kind === "source"
      ? typeof candidate.itemBucketIdentity === "function"
      : typeof candidate.validate === "function") &&
    typeof candidate.meta?.url === "string"
  );
}

/** Plugin descriptor associated with each resolved configuration object. */
const pluginReferences = new WeakMap<object, AnyPlugin>();

/** Returns the plugin descriptor associated with a resolved configuration node. */
export function pluginFor(
  value: object,
): AnyPlugin {
  /** Plugin descriptor previously attached to the configuration node. */
  const plugin = pluginReferences.get(value);
  if (!plugin) throw new TypeError("configuration node has no Plugin Descriptor");
  return plugin;
}

/** Copies a plugin association when code replaces a resolved configuration object. */
export function copyPluginReference(from: object, to: object): void {
  pluginReferences.set(to, pluginFor(from));
}

/** Disallows unknown fields on object schemas that do not set an explicit policy. */
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

/** Separates the configuration node identifier from the plugin payload. */
function splitConfig(config: unknown): { id: string | undefined; payload: unknown } {
  if (config === null || typeof config !== "object" || Array.isArray(config)) {
    return { id: undefined, payload: config };
  }

  /** Node identifier and schema payload extracted from the input object. */
  const { id, ...payload } = config as Record<string, unknown>;
  if (id !== undefined && typeof id !== "string") {
    throw new TypeError("configuration id must be a string");
  }
  return { id, payload };
}

/** Defaults, validates, identifies, and associates one plugin configuration node. */
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

  /** Optional node identifier and plugin-owned configuration payload. */
  const { id, payload } = splitConfig(config);
  /** Configuration payload after TypeBox applies schema defaults. */
  const normalized = Compile(plugin.schema).Default(payload);
  /** Configuration payload after strict schema validation. */
  const validated = Compile(strictPluginSchema(plugin.schema)).Parse(
    normalized,
  ) as Static<Schema>;
  /** Provisional location normalized when the workspace loads. */
  const identity = {
    path,
    role: plugin.kind,
    position: 0,
  } as const;
  /** Resolved node returned to executable workspace configuration code. */
  const resolved = {
    kind: plugin.kind,
    id,
    identity,
    config: validated,
  } as ResolvedConfiguration<Role, Schema>;

  pluginReferences.set(resolved, plugin as AnyPlugin);
  return resolved;
}

/** Resolves one source plugin configuration. */
export function source<const Schema extends TSchema, Runtime extends SourceRuntime>(
  plugin: Plugin<"source", Schema, Runtime>,
  config: ConfigWithId<Schema>,
  path: string,
): ResolvedSource<Schema> {
  return resolve("source", plugin, config, path);
}

/** Resolves one action plugin configuration. */
export function action<const Schema extends TSchema, Runtime extends PreparedActionRuntime>(
  plugin: Plugin<"action", Schema, Runtime>,
  config: ConfigWithId<Schema>,
  path: string,
): ResolvedAction<Schema> {
  return resolve("action", plugin, config, path);
}

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

/** State of one Item in a configured pipeline. */
export type PipelineState =
  | "claimed"
  | "running"
  | "succeeded"
  | "failed"
  | "cancelled"
  | "stale";

/** AgentBoard orchestration policy for one Source. */
export interface PipelineConfig {
  /** Maximum number of new Item claims in one Run. */
  readonly claim_limit?: number;
}

/** Result returned after an action completes. */
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

/** Values supplied when AgentBoard creates an Action runtime. */
export interface ActionRuntimeCreationContext {
  /** Workspace that owns the configured Action. */
  readonly workspaceId: string;
  /** Configured Source that owns the Action. */
  readonly sourceId: string;
  /** Signal that requests cancellation of runtime setup. */
  readonly cancellation: AbortSignal;
}

/** Values supplied when an Action runtime processes one Item. */
export interface ActionExecutionContext<Inputs = unknown> {
  /** Workspace that owns the action run. */
  readonly workspaceId: string;
  /** Configured source that produced the item. */
  readonly sourceId: string;
  /** Item that the action must process. */
  readonly item: Item;
  /** Rendered Plugin inputs for this Action execution. */
  readonly inputs: Inputs;
  /** Signal that requests cancellation of action work. */
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


/** An Action launch that has not reached a final outcome. */
export interface ActionInProgress {
  /** Indicates that launch was accepted and completion is pending. */
  readonly outcome: "running";
  /** Output available at launch time. */
  readonly stdout: string;
  /** Error output available at launch time. */
  readonly stderr: string;
  /** Optional launch detail. */
  readonly message?: string;
  /** Final result when the Action can observe completion. */
  readonly completion?: Promise<ActionResult>;
}

/** Result returned by an Action execution. */
export type ActionExecutionResult = ActionResult | ActionInProgress;

/** Workspace-scoped runtime that executes an Action for normalized Items. */
export interface ActionRuntime<Inputs = unknown> {
  /** Processes one item and returns a final result or an in-progress launch. */
  execute(context: ActionExecutionContext<Inputs>): Promise<ActionExecutionResult> | ActionExecutionResult;
  /** Reports whether an earlier successful result can be reused. */
  cachedSuccessIsValid?(context: ActionExecutionContext<Inputs>): Promise<boolean> | boolean;
}

/** A value that a Plugin can return now or through a Promise. */
type MaybePromise<Value> = Value | PromiseLike<Value>;

/** Runtime contract selected for a plugin role. */
type RuntimeFor<Role extends PluginRole, Inputs = unknown> = Role extends "source"
  ? SourceRuntime
  : ActionRuntime<Inputs>;

/** Fields shared by Source and Action Plugin Descriptors. */
interface PluginBase<
  Role extends PluginRole,
  Schema extends TSchema,
> {
  /** Role performed by the plugin. */
  readonly kind: Role;
  /** TypeBox schema used to default and validate plugin configuration. */
  readonly schema: Schema;
  /** Checks whether the configured plugin can run. */
  readonly healthCheck: (
    config: Static<Schema>,
    context: HealthCheckContext,
  ) => unknown;
  /** Configuration paths that AgentBoard resolves as filesystem inputs. */
  readonly pathInputs?: readonly string[];
  /** Module metadata added by {@link definePlugin}. */
  readonly meta: PluginMeta;
}

/** Complete descriptor for a Source or Action Plugin. */
export type Plugin<
  Role extends PluginRole,
  Schema extends TSchema,
  Runtime extends RuntimeFor<Role, Static<Schema>> = RuntimeFor<Role, Static<Schema>>,
> = Role extends "source"
  ? PluginBase<Role, Schema> & {
    /** Creates the Source runtime from validated configuration. */
    readonly runtime: (
      config: Static<Schema>,
      context: SourceRuntimeContext,
    ) => MaybePromise<Runtime>;
    /** Returns the Store bucket identity for Source Items. */
    readonly itemBucketIdentity: (config: Static<Schema>) => string;
    readonly validate?: never;
  }
  : PluginBase<Role, Schema> & {
    /** Creates one Action runtime for the loaded Workspace. */
    readonly runtime: (
      config: Static<Schema>,
      context: ActionRuntimeCreationContext,
    ) => MaybePromise<Runtime>;
    readonly itemBucketIdentity?: never;
    /** Applies Action-specific validation after schema validation. */
    readonly validate?: (config: Static<Schema>) => unknown;
  };

/** Author-supplied Plugin fields before AgentBoard adds module metadata. */
type PluginDefinition<
  Role extends PluginRole,
  Schema extends TSchema,
  Runtime extends RuntimeFor<Role, Static<Schema>>,
> = Omit<Plugin<Role, Schema, Runtime>, "meta">;

/**
 * Defines a plugin and records the defining module URL.
 *
 * Runtime checks keep JavaScript callers from bypassing the role-specific type rules.
 */
export function definePlugin<const Schema extends TSchema, Runtime extends SourceRuntime>(
  module: Pick<ImportMeta, "url">,
  definition: PluginDefinition<"source", Schema, Runtime>,
): Plugin<"source", Schema, Runtime>;

export function definePlugin<const Schema extends TSchema, Runtime extends ActionRuntime<Static<Schema>>>(
  module: Pick<ImportMeta, "url">,
  definition: PluginDefinition<"action", Schema, Runtime>,
): Plugin<"action", Schema, Runtime>;

export function definePlugin(
  module: Pick<ImportMeta, "url">,
  definition: PluginDefinition<PluginRole, TSchema, RuntimeFor<PluginRole, unknown>>,
): Plugin<PluginRole, TSchema> {
  if (definition.kind !== "source" && definition.kind !== "action") {
    throw new TypeError(`invalid plugin role: ${String(definition.kind)}`);
  }
  if (typeof definition.healthCheck !== "function") {
    throw new TypeError("plugin must define healthCheck()");
  }
  if (definition.kind === "source" && typeof definition.itemBucketIdentity !== "function") {
    throw new TypeError("source plugin must define itemBucketIdentity()");
  }
  if (typeof definition.runtime !== "function") {
    throw new TypeError("plugin must define runtime()");
  }
  if (definition.kind === "action" && typeof definition.validate !== "function") {
    throw new TypeError("action plugin must define validate()");
  }
  return {
    ...definition,
    meta: { url: module.url },
  } as Plugin<PluginRole, TSchema>;
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
> & {
  /** Optional command used to open the Item associated with this Action. */
  readonly open?: string;
};

/** Executable configuration for one AgentBoard workspace. */
export interface WorkspaceConfig {
  /** Source configurations and their attached actions. */
  readonly sources: readonly WorkspaceSourceConfig[];
}

/** One configured source and the actions that process its items. */
export interface WorkspaceSourceConfig {
  /** Workspace-local identifier for the source. */
  readonly id: string;
  /** AgentBoard orchestration policy for the Source. */
  readonly pipeline?: PipelineConfig;
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

/** Optional metadata for one resolved Action configuration. */
export interface ActionOptions {
  readonly open?: string;
}

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
    typeof candidate.healthCheck === "function" &&
    typeof candidate.runtime === "function" &&
    (candidate.kind === "source"
      ? typeof candidate.itemBucketIdentity === "function"
      : (candidate.validate === undefined || typeof candidate.validate === "function")) &&
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
  Runtime extends RuntimeFor<Role, Static<Schema>>,
>(
  expectedRole: Role,
  plugin: Plugin<Role, Schema, Runtime>,
  config: ConfigWithId<Schema>,
  path = "",
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
  path?: string,
): ResolvedSource<Schema> {
  return resolve("source", plugin, config, path);
}

/** Resolves one action plugin configuration. */
export function action<const Schema extends TSchema, Runtime extends ActionRuntime<Static<Schema>>>(
  plugin: Plugin<"action", Schema, Runtime>,
  config: ConfigWithId<Schema>,
  pathOrOptions?: string | ActionOptions,
  options?: ActionOptions,
): ResolvedAction<Schema> {
  const path = typeof pathOrOptions === "string" ? pathOrOptions : undefined;
  const metadata = options ?? (typeof pathOrOptions === "object" ? pathOrOptions : undefined);
  const resolved = resolve("action", plugin, config, path);
  return metadata?.open === undefined ? resolved : { ...resolved, open: metadata.open };
}

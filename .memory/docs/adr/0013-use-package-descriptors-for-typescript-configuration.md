# Use package descriptors for TypeScript configuration

**Status:** accepted

AgentBoard will use Bun to load `agentboard.config.ts` and `agentboard.config.js`. Each AgentBoard package will export one Plugin Descriptor through `definePlugin(import.meta, { kind, schema, runtime, healthCheck })`. The TypeBox schema will describe Plugin data only.

The core `source()` and `action()` functions will create resolved configuration nodes from Plugin Descriptors. TypeScript configuration will keep a private Plugin reference and will not require `uses`. JSON, YAML, and TOML configuration will require `uses` with the exact package name, resolve the package, validate its data, and create the same resolved node. AgentBoard will derive the package name for Store records and diagnostics. Inline Plugins will use the config path, role, and position as their identity.

The CLI will load local packages before global packages from `~/.local/share/agentboard/plugins/npm/`. Normal configuration loading will import only selected packages. `agentboard schema` will import all available packages. Packages outside project code must include the `agentboard-package` keyword. Missing packages will fail with an install command. Package renames are breaking changes.

## Consequences

- Plugin packages own their schema and runtime behavior.
- TypeScript configuration does not repeat package names in every Source and Action.
- Data configuration remains self-describing and can support editor schema validation.
- Inline Plugins have identity that changes when their config path, role, or position changes.
- Executable configuration runs with normal Bun permissions.

# Use package descriptors for TypeScript configuration

**Status:** accepted

AgentBoard will use Bun to load `agentboard.config.ts` and `agentboard.config.js`. Each Source package will export a Plugin Descriptor with `runtime`. Each Action package will export a Plugin Descriptor with `prepare`. Both roles will include `kind`, `schema`, and `healthCheck`. The TypeBox schema will describe Plugin data only. ADR 0014 defines the Action lifecycle.

The core `source()` and `action()` functions will create resolved configuration nodes from Plugin Descriptors. TypeScript configuration will keep a private Plugin reference and will not require `uses`. JSON, YAML, and TOML configuration will require `uses` with the exact package name, resolve the package, validate its data, and create the same resolved node. AgentBoard will derive the package name for Store records and diagnostics. Inline Plugins will use the config path, role, and position as their identity.

The CLI will load local packages before global packages from `~/.local/share/agentboard/plugins/npm/`. Normal configuration loading will import only selected packages. `agentboard schema` will import all available packages. Packages outside project code must include the `agentboard-package` keyword. Missing packages will fail with an install command. Package renames are breaking changes.

During the Rust-to-Bun migration, canonical Rust TOML files will keep `kind`. Bun data-loader tests will use separate fixtures with `uses`. When issue #57 makes Bun the default, canonical TOML files and documentation will change to `uses`. The retained Rust CLI will not read Plugin-backed configuration after that change.

## Consequences

- Plugin packages own their schema and runtime behavior.
- TypeScript configuration does not repeat package names in every Source and Action.
- Data configuration remains self-describing and can support editor schema validation.
- Inline Plugins have identity that changes when their config path, role, or position changes.
- A package rename or inline Plugin identity change creates a new Store identity, as defined by ADR 0015.
- Executable configuration runs with normal Bun permissions.

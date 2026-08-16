# Plugin-backed Workspace configuration

## Scope boundary

- Implement production code in TypeScript.
- Run the implementation and tests with Bun.
- Treat JavaScript configuration support as an input compatibility case handled by the TypeScript loader. Do not add a separate JavaScript implementation.
- Do not edit Rust files, Rust tests, `Cargo.toml`, `Cargo.lock`, or Rust task configuration.
- Do not port, migrate, or remove the existing Rust implementation.


## Problem Statement

AgentBoard currently describes Source and Action configuration through closed schema unions. Each new package requires central schema imports and central registration changes.

This prevents package discovery and makes configuration types depend on the complete set of built-in packages.

AgentBoard also needs one configuration model for TypeScript, JavaScript, JSON, YAML, and TOML files.

## Solution

Use Bun to load executable Workspace configuration from `agentboard.config.ts` or `agentboard.config.js`.

Each Source package exports one Plugin Descriptor with `kind`, `schema`, `runtime`, and `healthCheck`.

Each Action package exports one Plugin Descriptor with `kind`, `schema`, `runtime`, and `healthCheck`.

The Plugin schema is a TypeBox payload schema. It excludes core fields such as `uses`, Source IDs, Action IDs, and the Action `with` wrapper.

Core exposes `source(plugin, config)` and `action(plugin, config)` constructors. These constructors validate Plugin data, apply schema defaults, check the Plugin role, and return resolved configuration nodes.

TypeScript configuration keeps a private Plugin Descriptor reference. It does not require a `uses` field.

JSON, YAML, and TOML configuration requires `uses` for every Source and Action. The data loader resolves the package, validates its payload, and creates the same resolved configuration nodes.

The runtime consumes resolved nodes. AgentBoard creates one Source runtime for each configured Source and one Action runtime for each configured Action. Both runtimes live for the loaded Workspace. AgentBoard renders Action inputs before each execution.

## User Stories

1. As a Workspace author, I want TypeScript type inference for Source configuration, so that invalid fields fail during development.
2. As a Workspace author, I want TypeScript type inference for Action configuration, so that invalid Action inputs fail during development.
3. As a Workspace author, I want TypeScript configuration to import Plugin Descriptors, so that I do not repeat package names in every Source and Action.
4. As a Workspace author, I want `source(plugin, config)` to create Source configuration, so that Source role checks remain in core.
5. As a Workspace author, I want `action(plugin, config)` to create Action configuration, so that Action role checks remain in core.
6. As a Workspace author, I want TypeBox defaults applied during normalization, so that TypeScript and data configuration produce equal values.
7. As a Workspace author, I want inline Plugins, so that I can test local Source and Action behavior without publishing a package.
8. As a Workspace author, I want inline Plugin identity derived from config path, role, and position, so that AgentBoard can store execution records.
9. As a data configuration author, I want `uses` to identify each Source and Action package, so that AgentBoard can load the required package.
10. As a data configuration author, I want Plugin payload validation, so that invalid fields fail before a Run starts.
11. As a data configuration author, I want the same defaults and runtime behavior as TypeScript configuration, so that file format does not change execution.
12. As a package author, I want one default Plugin Descriptor per package, so that package discovery has one clear result.
13. As a package author, I want `definePlugin()` to capture module metadata, so that I do not repeat the package name in source code.
14. As a package author, I want a TypeBox schema to define Plugin input, so that one schema drives types, defaults, and runtime validation.
15. As a package author, I want one runtime callback for every Plugin role. This gives configured Plugin resources one clear Workspace lifetime. Each Action execution receives its rendered inputs.
16. As a package author, I want role-specific runtime contexts, so that Source and Action packages do not depend on the full CLI.
17. As a package author, I want a required health check, so that `doctor` can validate every Plugin.
18. As a package author, I want Source runtimes to return normalized Items, so that each Source owns provider-specific normalization.
19. As a package author, I want Action runtimes to return AgentBoard Action Results, so that the CLI can store outcomes consistently.
20. As a package author, I want external package loading to require `agentboard-package`, so that unrelated packages are not treated as Plugins.
21. As a package author, I want one package to provide one Plugin, so that package identity remains direct.
22. As a CLI user, I want local packages to override global packages, so that project dependencies control execution.
23. As a CLI user, I want global packages under the AgentBoard Plugin directory, so that I can share packages across projects.
24. As a CLI user, I want normal configuration loading to import selected packages only, so that unrelated package code does not execute.
25. As a CLI user, I want `agentboard schema` to load all available packages, so that generated data schemas include every installed Plugin.
26. As a CLI user, I want missing packages to produce an install command, so that I can correct configuration without an unclear module error.
27. As a CLI user, I want executable configuration to take precedence over `.agentboard.toml`, so that the new configuration format is the default migration path.
28. As a CLI user, I want executable configuration to run with normal Bun permissions, so that trusted configuration can use normal JavaScript tooling.
29. As a CLI user, I want Source failures to remain Source-scoped, so that other Sources can continue a Run.
30. As a CLI user, I want Action failures to remain Item-scoped, so that one failed Action does not stop other Items.
31. As a CLI user, I want configuration and Plugin runtime creation errors to fail Workspace loading, so that AgentBoard does not run with invalid configuration.
32. As a CLI user, I want package names in Store records and diagnostics, so that execution history remains readable.
33. As a CLI user, I want package renames to be explicit breaking changes, so that Store identity does not change silently.

## Implementation Decisions

- Use one high-level seam: configuration normalization. Executable and data loaders both produce resolved configuration nodes before runtime orchestration.
- Define role-specific Plugin runtime interfaces. Every Plugin exposes `runtime`, `kind`, a TypeBox `schema`, and `healthCheck`. The Plugin role selects the runtime interface.
- Pass `import.meta` to `definePlugin()` so the package loader can resolve the nearest package metadata without a repeated package name.
- Treat a package marked with `agentboard-package` as an external Plugin Package. Permit inline Plugins in project code.
- Require one Plugin per package. Use the exact `package.json.name` as the package identity.
- Use `uses` as the serialized discriminator for both Sources and Actions.
- Keep Source payload fields flat. Keep Action payload fields inside `with`.
- Keep Source IDs outside Source Plugin payload. Keep optional Action IDs as core metadata. The `action()` constructor removes the Action ID before Plugin schema validation.
- Use TypeBox schemas for Plugin payloads. Apply defaults and validate with one shared validation engine.
- Make Plugin runtime creation role-specific. Source creation returns collection behavior. Action creation returns execution behavior.
- Create one Source runtime per configured Source and one Action runtime per configured Action in a loaded Workspace.
- Render Action inputs before execution and pass them to the Workspace-scoped Action runtime.
- In Watch Mode, load one Workspace and reuse all Plugin runtimes for every Run cycle. Configuration changes require a command restart.
- Permit Source and Action runtime creation to return synchronous or asynchronous results. Workspace loading awaits both stages.
- Require Plugin health checks on the Plugin Descriptor. Run them without creating Plugin runtimes.
- Keep Source runtime output as normalized Items with raw provider data. Keep Action runtime output as an AgentBoard Action Result.
- Preserve current error scope. Configuration and Plugin runtime creation errors fail Workspace loading. Source collection errors remain Source-scoped. Action rendering and execution errors remain Item-scoped. A failed Action stops later Actions for that Item. Other Items and Sources continue.
- Resolve local packages before global packages under `~/.local/share/agentboard/plugins/npm/`.
- Import selected packages during normal config loading. Import all available packages for `agentboard schema`.
- Require `agentboard.config.ts` or `agentboard.config.js` before `.agentboard.toml` when both default files exist.
- During issues #48 through #56, keep canonical Rust TOML files on `kind`. Use separate Bun data-loader fixtures with `uses`.
- During issue #57, change canonical TOML files and documentation to `uses`. The retained Rust CLI does not need to read Plugin-backed configuration.
- Use the nearest ancestor `package.json` from the config file for local package discovery.
- Fail when a data configuration names a missing package. Report an install command. Do not install packages automatically.
- Treat package renames as breaking changes. A package rename creates a new Store identity. Do not connect new Store records to old records.
- Derive inline Plugin identity from config path, role, and position. An inline Plugin identity change creates a new Store identity.
- Store package identity in diagnostics and execution records after resolution.
- Run executable configuration with normal Bun permissions.

## Testing Decisions

- Test external behavior at the configuration loader, registry, and runtime orchestration seams. Do not test private helper structure.
- Use TypeScript type checks to verify that `source()` and `action()` infer payload types from TypeBox schemas.
- Test schema defaults, unknown fields, missing fields, and role mismatch errors.
- Test TypeScript configuration and data configuration with equivalent payloads. Assert that both produce equivalent resolved configuration.
- Test data configuration rejection when `uses` is missing or names an unknown package.
- Test package discovery for local precedence, global fallback, keyword enforcement, one Plugin per package, and duplicate package identity.
- Test lazy package loading during normal configuration and full package loading during `agentboard schema`.
- Test inline Plugin identity changes when config path, role, or position changes.
- Test Source runtime lifetime across repeated Workspace Runs.
- Test Action runtime lifetime across repeated Workspace Runs.
- Test that Watch Mode reuses one loaded Workspace until cancellation.
- Test that Action execution receives rendered inputs.
- Test that Plugin runtime creation errors fail Workspace loading.
- Test synchronous and asynchronous Source and Action runtime creation.
- Test required health checks without creating runtimes.
- Test Source errors remain Source-scoped and Action errors remain Item-scoped.
- Test executable configuration precedence and missing-package install messages.
- Reuse the existing CLI Bats patterns for command behavior and schema validation. Reuse current Registry and runtime tests for registration, validation, error scope, and execution behavior.

## Out of Scope

- Automatic package installation.
- A stable third-party runtime ABI.
- Loading multiple Plugins from one package.
- Plugin package aliases or rename migration.
- Configuration sandboxing.
- A TypeScript schema output for executable configuration.
- Changes to Source query semantics or Action side effects.
- Changes to Store record format beyond derived Plugin identity.
- Rust code changes, Rust CLI migration, Rust runtime integration, and Rust task or configuration changes.
- Explicit disposal for Plugin runtimes.


## Dependency graph

- [x] #48
  - [x] #49
    - [x] #50
    - [x] #51
    - [ ] #52 — blocked by #50 and #51
      - [ ] #53
      - [ ] #54
      - [ ] #55
      - [ ] #56
      - [ ] #57 — blocked by #53, #54, #55, and #56

## Further Notes

The accepted Plugin Descriptor decision is recorded in the system ADR for TypeScript configuration. This spec defines the implementation contract. Split it into tracer-bullet issues before implementation.

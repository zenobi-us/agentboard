# Research: Rust plugin interfaces and registry for AgentBoard Actions and Sources

Historical snapshot: “current” enum/match descriptions below are pre-ADR-0010 evidence. The adopted implementation is one explicit static Registry with no runtime plugin ABI; see ADR 0010 and current Core/CLI context docs.

## Summary

AgentBoard should adopt a compile-time plugin seam using one explicitly populated `Registry` with separate source and action maps. Generic typed plugin definitions provide IDs, typed configuration, generated schemas, and constructors; registration erases those definitions into factories, while small runtime trait objects provide source and action behavior.

This is Candidate B below. Keep registration explicit and statically linked. Do not use `inventory`, proc macros, `async-trait`, or dynamic libraries for the first registry. They remove a few explicit registration lines while adding hidden startup behavior, allocation/macro machinery, or an ABI problem AgentBoard does not currently have.

## Findings

1. **Current AgentBoard already has a compile-time registry, expressed as enums and matches** — `SourceKind` is a closed, Serde-tagged enum, workspace schema comes from `schemars::schema_for!(WorkspaceConfig)`, validation matches each source kind, collection dispatch matches each source kind, and action validation/execution matches `uses`. Source failures are aggregated independently while source pipelines run concurrently. See `pkgs/crates/agentboard-core/src/model.rs::{WorkspaceConfig,SourceConfig,SourceKind,ActionConfig}`, `apps/cli/src/cli.rs::Command::Schema`, `apps/cli/src/config.rs::{validate_config,validate_action}`, `apps/cli/src/adapters.rs::{collect_items,inspect_source,execute_action}`, and `apps/cli/src/runtime.rs::{run_sources,run_source}`. ADR 0008 explicitly records existing dispatch as `SourceKind` plus CLI matches: `.memory/docs/adr/0008-use-one-github-source-with-explicit-modes.md::{Context,Decision}`.

2. **Rust has no `static` method keyword; the sketch's `static defineConfigSchema()` is an associated function** — a trait function without a `self` receiver is not dispatchable through `dyn Trait`. To keep such a trait dyn-compatible, the associated function must be constrained with `where Self: Sized`, after which it is callable only through a concrete implementor such as `<MySourcePlugin as SourcePlugin>::config_schema()`. Associated constants and associated types also prevent using the definition trait itself as a trait object. This is useful here: keep the typed plugin-definition trait static and erase it only when registering. [Rust Reference: traits and dyn compatibility](https://doc.rust-lang.org/reference/items/traits.html#dyn-compatibility)

3. **Constructors belong in registration factories, not runtime trait objects** — `fn new(config: Config) -> Self` has no `self` receiver and returns `Self`, so it is a concrete-type operation rather than a `dyn Source` operation under Rust's dyn-compatibility rules. A generic `Registry::add_source::<P>()` can monomorphize a factory into `fn(Value) -> Result<Box<dyn Source>>`; the registry then stores that ordinary function pointer. [Rust Reference: dyn compatibility](https://doc.rust-lang.org/reference/items/traits.html#dyn-compatibility) Trait objects contain a pointer to the value and a vtable for the concrete implementation's methods; they are appropriate for the constructed runtime behavior, not type-level metadata. [Rust Reference: trait object types](https://doc.rust-lang.org/reference/types/trait-object.html)

4. **Native `async fn` in traits does not solve trait-object dispatch** — native `async fn` in traits stabilized in Rust 1.75, but the stabilized form did not add dynamic dispatch, and the language's current dyn-compatibility rules reject `async fn` methods and methods returning opaque `impl Trait` from dyn-compatible traits. [Rust announcement](https://blog.rust-lang.org/2023/12/21/async-fn-rpit-in-traits/) [Rust Reference: dyn compatibility](https://doc.rust-lang.org/reference/items/traits.html#dyn-compatibility) AgentBoard sources need async collection because Jira and GitHub already await HTTP calls (`pkgs/crates/agentboard-source-jira/src/lib.rs::{collect_items,collect_jira,jira_search}` and `pkgs/crates/agentboard-source-github/src/lib.rs::{collect_items,collect_github_issues,github_issue_search}`). The dependency-free object-safe signature is conceptually:

   ```rust
   fn collect<'a>(
       &'a self,
       context: &'a SourceContext,
   ) -> Pin<Box<dyn Future<Output = Result<Vec<Item>>> + Send + 'a>>;
   ```

   `Future::poll` itself uses `Pin<&mut Self>`, which is why erased futures are normally pinned. [std::future::Future](https://doc.rust-lang.org/std/future/trait.Future.html) `async-trait` can generate equivalent boxed-future plumbing and supports dyn traits, but it is a new dependency and macro transformation, not a capability AgentBoard needs to hand-write more than once. [async-trait documentation](https://docs.rs/async-trait/latest/async_trait/)

5. **Typed config and object-safe runtime behavior should be separate layers** — each compile-time plugin definition can expose `type Config: DeserializeOwned + JsonSchema`, an ID, and a typed constructor. `schemars::schema_for!(T)` generates a root schema for a concrete `JsonSchema` type. [Schemars `schema_for!`](https://docs.rs/schemars/0.8.22/schemars/macro.schema_for.html) The generic registrar supplies erased schema and factory function pointers. Plugin config structs should use Serde's `deny_unknown_fields` at their own boundary, matching current core config (`pkgs/crates/agentboard-core/src/model.rs::{WorkspaceConfig,SourceConfig,ActionConfig}`). Do not model the flat source table with Serde `flatten` plus `deny_unknown_fields`; Serde explicitly does not support that combination. Extract `kind` from the raw table first, then deserialize the remaining table into the selected plugin's typed config. [Serde flatten](https://serde.rs/attr-flatten.html) [Serde container attributes](https://serde.rs/container-attrs.html#deny_unknown_fields)

6. **Schema generation becomes the main cost of an open registry** — today `WorkspaceConfig` is a closed enum and `Command::Schema` derives the whole schema in one expression (`apps/cli/src/cli.rs::Command::Schema`). An open compile-time registry must instead assemble source variants from registered schemas, add each source ID as a literal/constant discriminator, merge or namespace each root schema's definitions, and similarly build action `uses` variants. Runtime deserialization is straightforward; preserving a useful full-workspace schema is not. Keeping both `SourceKind` and a registry would avoid schema composition but create two authorities for IDs, config, validation, and dispatch. That duplication is worse than either complete design.

7. **One registry can preserve hard source/action separation** — use one owner with two private maps and category-specific APIs:

   ```text
   Registry
   +-- sources: Map<SourceId, SourceRegistration>
   |   +-- schema
   |   `-- typed-config-erasing factory -> dyn Source
   `-- actions: Map<ActionId, ActionRegistration>
       +-- schema
       `-- typed-config-erasing factory -> dyn Action
   ```

   `add_source::<P>()` cannot insert an action definition, and `add_action::<P>()` cannot insert a source definition. IDs need only be unique within their category; duplicate insertion must return an error and abort startup rather than overwrite. Unknown IDs must fail workspace loading before any collection or side effect. This retains the project boundaries in `CONTEXT-MAP.md::{Boundaries,System flow}`, `apps/cli/CONTEXT.md::Boundaries`, and the source/action package `CONTEXT.md` files.

8. **Factories should parse config; runtime traits should perform only domain behavior** — source construction should be synchronous and side-effect-free: parse typed config and enforce semantic invariants, then return a source instance. Credential lookup, network access, and command execution belong in `collect`, as they do now in source crates. Action config validation should happen at workspace load, but construction/execution must account for CLI-owned template rendering (`apps/cli/CONTEXT.md::Boundaries`, `apps/cli/src/runtime.rs::run_source`, and `apps/cli/src/template.rs::render_action`). Current action `with` values are strings, so typed action config should initially remain structs of templatable strings; non-string action config is a separate config-language change.

9. **Error boundaries need one normalization point** — registry errors (invalid/duplicate IDs) are process-startup failures; unknown plugin IDs and invalid typed config are workspace-load failures; source collection errors remain source-scoped so sibling sources complete; expected action construction/execution errors become failed `ActionAttempt`s for that item/action. Current action dispatch is inconsistent: `agentboard/run-cmd` errors escape `execute_action` through `?`, while worktree errors are converted into a failed `ActionRun` (`apps/cli/src/adapters.rs::execute_action`). A common action interface should make both paths item/action-scoped in `run_source`, consistent with root `AGENTS.md` guidance to prefer item-scoped failures. Panics should not be caught: these are trusted, statically linked crates, and panic recovery adds a false isolation boundary.

10. **Automatic compile-time registration is viable but wrong for this repository** — `inventory` collects values submitted from linked crates and iterates them as a distributed registry, but iteration order is not guaranteed. [inventory documentation](https://docs.rs/inventory/latest/inventory/) [inventory iterator](https://docs.rs/inventory/latest/inventory/type.iter.html) AgentBoard would still need deterministic duplicate detection and sorting, and every plugin crate still must be linked into the final CLI binary. Explicit calls such as `registry.add_source::<Github>()` expose the build composition in one place and require no dependency, linker constructors, or macro-based submission.

11. **Runtime/dynamic loading is not justified** — all adapters are workspace members and direct path dependencies of `apps/cli/Cargo.toml`; no current config or ADR defines third-party binary discovery, ABI versioning, unload/reload, or trust isolation. Rust trait objects are Rust-ABI values, not a stable cross-dynamic-library contract. A real dynamic plugin system needs an explicit C ABI or a compatibility layer such as `abi_stable`, which exists specifically to provide Rust-to-Rust FFI-safe types and trait objects. [Rust Reference: ABI](https://doc.rust-lang.org/reference/abi.html) [abi_stable documentation](https://docs.rs/abi_stable/latest/abi_stable/) Dynamic loading also introduces unsafe symbol lookup and library-lifetime constraints. [libloading documentation](https://docs.rs/libloading/latest/libloading/) This complexity buys nothing while AgentBoard actions can already invoke arbitrary trusted local commands (`README.md::GitHub Issues quickstart`, `pkgs/crates/agentboard-action-run-cmd/CONTEXT.md::Boundaries`).

## Design candidates

### Candidate A — keep closed enums and explicit matches

Shape:

- Keep `SourceKind` and `ActionConfig.uses`.
- Keep per-kind validation and dispatch in CLI matches.
- Add variants and match arms when adding built-ins.
- Treat existing match sites as the explicit registry.

Trade-offs:

| Concern | Result |
| --- | --- |
| Size | Smallest; zero new traits, maps, factories, dependencies, or schema assembly. |
| Type safety | Strongest whole-workspace compile-time types. |
| Async | Inherent async functions; no trait-object issue. |
| Config schema | Existing derive remains complete and simple. |
| Duplicate plugin IDs | Impossible at registration; enum variant names and match literals are compile-time source. Source instance IDs remain checked by `validate_config`. |
| Extension cost | New built-in touches core config, CLI validation/dispatch, dependency wiring, and adapter crate. |
| Runtime loading | None. |

Verdict: valid status quo and useful migration baseline, but rejected for the chosen direction because adding a built-in continues to require central config and dispatch edits.

### Candidate B — explicit typed registration with erasure at registry boundary

Conceptual shape, not proposed implementation:

```rust
trait SourceDefinition {
    const ID: &'static str;
    type Config: DeserializeOwned + JsonSchema;
    type Runtime: Source;

    fn build(config: Self::Config) -> Result<Self::Runtime>;
}

trait Source: Send {
    fn collect<'a>(
        &'a self,
        context: &'a SourceContext,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Item>>> + Send + 'a>>;
}

trait ActionDefinition {
    const ID: &'static str;
    type Config: DeserializeOwned + JsonSchema;
    type Runtime: Action;

    fn build(config: Self::Config) -> Result<Self::Runtime>;
}

trait Action {
    fn execute(&self, context: &ActionContext) -> Result<ActionRun>;
}

struct Registry {
    sources: BTreeMap<&'static str, SourceRegistration>,
    actions: BTreeMap<&'static str, ActionRegistration>,
}
```

Registration stays explicit:

```text
register_builtins(registry)
  -> add_source::<Qmd>()
  -> add_source::<Jira>()
  -> add_source::<Github>()
  -> add_action::<RunCmd>()
  -> add_action::<CreateWorktree>()
```

`SourceDefinition` and `ActionDefinition` are intentionally not trait objects. Generic `add_*::<P>()` converts each concrete definition into an erased registration containing ID, schema function, and factory function. Only constructed `Source` and `Action` behavior uses trait objects.

Trade-offs:

| Concern | Result |
| --- | --- |
| Size | Moderate; one registry, two definition traits, two runtime traits, and schema assembly. No macros or new crates required. |
| Type safety | Plugin authors get concrete typed config; registry stores erased factories. |
| Async | One handwritten boxed-future signature for sources; actions stay synchronous until a real async action exists. |
| Config schema | Per-plugin schema is easy; composing the existing complete workspace schema is the hard part. |
| Duplicate plugin IDs | Detect in `add_source`/`add_action`; fail startup. Separate namespaces. |
| Construction | Raw table selects ID, remaining values deserialize to `P::Config`, then `P::build`. |
| Error boundaries | Central registry/config errors; source and action runtime errors remain separately normalized. |
| Runtime loading | None; plugins are linked crates registered explicitly. |

Verdict: recommended. AgentBoard has chosen a common registry seam so built-in adapters can provide their own typed configuration and runtime behavior without extending central dispatch matches. This is the minimal actual registry.

### Candidate C — boxed descriptor objects

Shape:

```text
Registry.add_source(MySourceDescriptor)
Registry.add_action(MyActionDescriptor)
```

Descriptor traits use receiver methods such as `id(&self)`, `config_schema(&self)`, and `build(&self, Value)`, making the descriptors dyn-compatible. Descriptors are usually zero-sized values; runtime source/action objects remain separate.

Trade-offs:

- Closest syntax to the proposed sketch.
- Avoids associated-function dyn-compatibility problems by turning metadata into receiver methods.
- Hides each concrete `Config` inside handwritten `build` and `config_schema` methods; compiler cannot enforce one standard `Config: DeserializeOwned + JsonSchema` association as directly as Candidate B.
- Adds descriptor trait-object allocation/lifetime concerns despite descriptors being static metadata.
- Still requires explicit registration and schema composition.

Verdict: viable, but Candidate B gives stronger typed config with less runtime indirection.

### Candidate D — distributed compile-time registry with `inventory`

Shape:

```text
plugin crate -> inventory::submit!(SourceRegistration { ... })
CLI startup  -> inventory::iter::<SourceRegistration>
```

Trade-offs:

- Plugin crates self-register without editing a central list.
- Still compile-time linked; it is not runtime plugin discovery.
- New dependency and linker/startup magic.
- Iteration order is unspecified, so registry construction must sort and detect duplicates before use.
- Harder to audit which built-ins ship in a binary.

Verdict: viable for a large ecosystem of linked feature crates; unjustified for three sources and two actions.

## Recommendation

Adopt Candidate B: compile-time plugin definitions, explicit registration, and erased runtime behavior.

```rust
pub fn register(registry: &mut Registry) -> Result<()> {
    registry.add_source::<MySourcePlugin>()?;
    registry.add_action::<MyActionPlugin>()?;
    Ok(())
}
```

```rust
pub trait SourcePlugin: Sized + 'static {
    const ID: &'static str;
    type Config: DeserializeOwned + JsonSchema;
    type Runtime: Source;

    fn build(config: Self::Config) -> Result<Self::Runtime>;
}

pub trait Source: Send + Sync {
    fn collect(
        &self,
        context: &SourceContext,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Item>>> + Send + '_>>;
}
```

Mirror this with separate `ActionPlugin` and `Action` traits. Use one `Registry` with two private maps:

```text
Registry
├── sources: ID -> schema + erased factory
└── actions: ID -> schema + erased factory
```

Implementation constraints:

1. Keep registration explicit in the CLI composition root.
2. Generate each plugin schema from `type Config` through `schemars`; plugin authors should not hand-write `config_schema()`.
3. Use separate source/action definition traits and registration methods so categories cannot be mixed.
4. Erase only registrations and constructed runtime behavior.
5. Hand-write the source boxed-future method; keep actions synchronous until an async action exists.
6. Reject duplicate IDs at registry construction and unknown IDs at workspace load.
7. Preserve the current flat TOML layout by extracting `kind` before typed plugin deserialization, not by combining `flatten` with `deny_unknown_fields`.
8. Do not add `inventory`, proc macros, or dynamic loading. A separately approved requirement must justify ABI, versioning, discovery, and trust machinery.

[bias: explicit composition over automatic registration; repository has few built-ins and prioritizes boring config.]

## Registration and execution flow

```text
binary startup
    |
    +--> explicit register_builtins(registry)
    |       |
    |       +--> add_source::<P>() --duplicate source ID--> fatal startup error
    |       |       `--> erase schema + typed factory into sources map
    |       |
    |       `--> add_action::<P>() --duplicate action ID--> fatal startup error
    |               `--> erase schema + typed factory into actions map
    |
    `--> frozen Registry
            |
workspace TOML
    |
    v
parse common envelope: source.id, source.kind, actions[].uses
    |
    +--unknown kind/uses------------------------------> workspace load error
    |
    v
lookup category-specific registration
    |
    v
typed config deserialize + semantic validation
    |
    +--invalid config---------------------------------> workspace load error
    |
    v
instantiate Source
    |
    v
collect() boxed async future
    |
    +--source error-----------------------------------> record/report source failure;
    |                                                  sibling Sources continue
    v
Vec<Item> -> sort -> append Store -> pending Actions
                                      |
                                      v
                              CLI renders `with`
                                      |
                                      v
                              instantiate/execute Action
                                      |
                     +----------------+----------------+
                     |                                 |
                  success                        expected error
                     |                                 |
                     v                                 v
             successful ActionAttempt          failed ActionAttempt;
                                                later Actions for that Item stop
```

## Sources

- Kept: [Rust Reference — Traits: dyn compatibility](https://doc.rust-lang.org/reference/items/traits.html#dyn-compatibility) — authoritative rules for associated functions, `Self`, async methods, and trait objects.
- Kept: [Rust Reference — Trait object types](https://doc.rust-lang.org/reference/types/trait-object.html) — authoritative trait-object representation and dispatch model.
- Kept: [Rust Blog — Announcing async fn and return-position impl Trait in traits](https://blog.rust-lang.org/2023/12/21/async-fn-rpit-in-traits/) — primary stabilization announcement and dynamic-dispatch limitation.
- Kept: [std::future::Future](https://doc.rust-lang.org/std/future/trait.Future.html) — authoritative pinned polling contract.
- Kept: [Schemars 0.8.22 `schema_for!`](https://docs.rs/schemars/0.8.22/schemars/macro.schema_for.html) — official crate API used by AgentBoard's installed 0.8 line.
- Kept: [Serde flatten](https://serde.rs/attr-flatten.html) and [container attributes](https://serde.rs/container-attrs.html#deny_unknown_fields) — official config-deserialization constraints.
- Kept: [inventory](https://docs.rs/inventory/latest/inventory/) — official library documentation for distributed compile-time registration.
- Kept: [abi_stable](https://docs.rs/abi_stable/latest/abi_stable/) and [libloading](https://docs.rs/libloading/latest/libloading/) — official library documentation establishing extra machinery required by dynamic Rust plugins.
- Kept: AgentBoard source, context docs, Cargo manifests, and ADR 0008 cited by path and symbol/section above — authoritative current architecture.
- Dropped: blogs, tutorials, forum answers, Stack Overflow, and Wikipedia — secondary sources unnecessary because language, library, and repository primary sources cover the question.

## Gaps

No external technical gap remains. Product decisions still open:

1. Does “plugin” mean statically linked built-in modularity, downstream binary composition, or runtime third-party loading? Current evidence supports only the first.
2. Must `agentboard schema` remain one complete workspace schema? Candidate B needs deliberate schema composition and definition namespacing.
3. Must Action `with` remain a string-only templating map? Typed non-string action config changes rendering and should not be smuggled into registry work.
4. Should a failed Action stop later Actions only for the current Item, as current `run_source` does, or follow a configurable fail-fast policy? Registry design should preserve current behavior until an ADR changes it.

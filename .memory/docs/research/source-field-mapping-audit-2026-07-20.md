# Research: Source-field mapping across AgentBoard crates

Historical snapshot: architecture statements about closed Source enums and CLI dispatch describe the pre-ADR-0010 implementation. Current code uses typed Source registrations and generic Registry runtime dispatch; mapping conclusions remain useful.

## Summary

No. The crates under `pkgs/crates/` do **not** inherit a common mapping behavior. `agentboard-core` shares a `FieldMap` data type and normalized `Item` model, but no source trait or shared mapping engine exists; each source adapter implements its own lookup and normalization, while action crates consume already-normalized/rendered data and expose no source-field mapping. All three currently implemented source adapters do support user-selected paths for `reference_id`, `title`, `status`, and `url`, but QMD calls the config table `map`, Jira/GitHub call it `field_map`, and their semantics remain adapter-specific. [`pkgs/crates/agentboard-core/src/model.rs` — `SourceKind`, `FieldMap`, `Item`; `apps/cli/src/adapters.rs` — `collect_items`, `execute_action`]

## Direct answer

- **Shared:** serializable config/model types (`SourceConfig`, `SourceKind`, `FieldMap`, `Item`) and a conventional `collect_items(&SourceConfig) -> Result<Vec<Item>>` function shape. [`pkgs/crates/agentboard-core/src/model.rs` — named types; each `agentboard-source-*/src/lib.rs` — `collect_items`]
- **Not shared/inherited:** path traversal, default source fields, field-fetch behavior, status normalization, identity selection, and raw-payload layout. Those are duplicated or specialized inside each source crate. [`pkgs/crates/agentboard-source-qmd/src/lib.rs` — `normalize_document`, `optional_mapped_field`; `pkgs/crates/agentboard-source-jira/src/lib.rs` — `normalize_issue`, `jira_fetch_fields`, `optional_mapped_field`; `pkgs/crates/agentboard-source-github/src/lib.rs` — `normalize_issue`, `mapped_field`, `mapped_status`]
- **Not applicable to every crate:** action crates are executors. They receive rendered input maps (and, for `run-cmd`, context objects) after normalization; they do not map source payloads into `Item`. [`pkgs/crates/agentboard-action-run-cmd/src/lib.rs` — `run_cmd`; `pkgs/crates/agentboard-action-worktree/src/lib.rs` — `create_worktree`; `pkgs/crates/agentboard-action-*/src/docs.md`]

## End-to-end config and model flow

```text
Workspace TOML
  |
  | toml::from_str::<WorkspaceConfig> + serde tagged SourceKind
  v
CLI load/validation
  |
  | explicit match on SourceKind
  v
CLI source dispatch -----> source crate collect_items(&SourceConfig)
                              |
                              | adapter-local path lookup/defaults/status policy
                              v
                         core::Item { normalized fields, raw }
                              |
                 +------------+-------------+
                 |                          |
                 v                          v
          append JSONL Store         MiniJinja item/source context
                                            |
                                            v
                                    rendered BTreeMap inputs
                                            |
                                            v
                                      action executor
```

1. `load_workspace_inner` reads TOML directly into `WorkspaceConfig`, whose `SourceKind` is a closed, internally tagged enum. `FieldMap` is only a four-option path holder; it has no lookup methods or normalization behavior. Unknown config fields are rejected by Serde. [`apps/cli/src/config.rs` — `load_workspace_inner`; `pkgs/crates/agentboard-core/src/model.rs` — `WorkspaceConfig`, `SourceKind`, `FieldMap`]
2. CLI validation and dispatch are explicit enum matches, not trait-object or generic adapter inheritance. `collect_items` calls one concrete crate for each source kind. [`apps/cli/src/config.rs` — `validate_config`; `apps/cli/src/adapters.rs` — `collect_items`, `inspect_source`]
3. Each source crate matches `SourceConfig.source` again and normalizes source records into the shared `Item` shape. Every current adapter preserves source-specific data under `Item.raw`. [`pkgs/crates/agentboard-source-qmd/src/lib.rs` — `collect_items`, `normalize_document`; `pkgs/crates/agentboard-source-jira/src/lib.rs` — `collect_items`, `normalize_issue`; `pkgs/crates/agentboard-source-github/src/lib.rs` — `inspect_items`, `normalize_issue`]
4. `runtime::run_source` appends normalized items, then `template::render_action` exposes the complete serialized `item` (including `raw`) and `source` to MiniJinja. It passes only rendered string inputs to action dispatch. [`apps/cli/src/runtime.rs` — `run_source`; `apps/cli/src/store.rs` — `append_items`; `apps/cli/src/template.rs` — `render_action`, test `templates_expose_reference_id_and_complete_source`; `apps/cli/src/adapters.rs` — `execute_action`]
5. Store and retry identity use adapter-owned `item.id`, not the mapped `reference_id`. Mapping `id` therefore selects the provider-facing `item.reference_id` only. [`pkgs/crates/agentboard-core/CONTEXT.md` — “Item identity” and “Item reference ID”; `apps/cli/src/store.rs` — `action_key`, `latest_item_records`; source tests `qmd_map_id_changes_reference_not_identity`, `jira_field_map_id_changes_reference_not_identity`, `supports_github_field_mapping`]

## Per-crate matrix

| Crate | Role | User mapping surface | Implemented behavior | Shared/inherited assessment |
|---|---|---|---|---|
| `agentboard-core` | Shared model/config | Defines `FieldMap { id, title, status, url }`; embeds it as QMD `map` and Jira/GitHub `field_map` | Serde/schema data only; defines normalized `Item` and closed `SourceKind` enum | **Shared type, no mapping behavior or source trait**. [`pkgs/crates/agentboard-core/src/model.rs` — `SourceKind`, `FieldMap`, `Item`] |
| `agentboard-source-qmd` | Source adapter | `[sources.source.map]` | Dot-path lookup is against parsed YAML frontmatter. Defaults: `id`, `title`, `status`, `url`; missing URL falls back to document reference. Document reference remains `item.id`; mapped `id` becomes `reference_id`. Raw stores QMD result, frontmatter, and body. | **Adapter-specific implementation**. [`pkgs/crates/agentboard-source-qmd/src/lib.rs` — `normalize_document`, `optional_mapped_field`; tests `supports_nested_field_mapping`, `qmd_map_id_changes_reference_not_identity`] |
| `agentboard-source-jira` | Source adapter | `[sources.source.field_map]`, plus separate `status_map` and `fields` | Dot-path lookup is against Jira issue JSON. Defaults use `key`, `fields.summary`, `fields.status.name`, and generated browse URL. Paths beginning `fields.` affect Jira API field selection; mapped values must be strings. Internal Jira `id` remains `item.id`; raw stores Jira issue JSON. | **Adapter-specific implementation using shared type**. [`pkgs/crates/agentboard-source-jira/src/lib.rs` — `normalize_issue`, `jira_fetch_fields`, `optional_mapped_field`; tests `infers_jira_fetch_fields_from_mapping_paths`, `jira_field_map_id_changes_reference_not_identity`, `maps_jira_status_values`] |
| `agentboard-source-github` | Source adapter | `[sources.source.field_map]`, plus required `status_map` | Dot-path lookup is against GitHub issue JSON. Defaults use issue number/title/state/HTML URL. Identity is `owner/repo#number`; mapped `id` becomes `reference_id`. Status mapping checks labels first, then state. Raw stores issue JSON. | **Adapter-specific implementation using shared type**. [`pkgs/crates/agentboard-source-github/src/lib.rs` — `normalize_issue`, `mapped_field`, `mapped_status`; tests `supports_github_field_mapping`, `normalizes_issue_identity_and_status_label`, `same_issue_number_in_different_repositories_has_distinct_identity`] |
| `agentboard-action-run-cmd` | Action executor | None | Runs already-rendered `cmd`/optional `cwd`; receives `Item` only to set `AGENTBOARD_ITEM_ID` and receives no raw-source mapping config. | **No source mapping; intentionally outside source boundary**. [`pkgs/crates/agentboard-action-run-cmd/src/lib.rs` — `run_cmd`; `pkgs/crates/agentboard-action-run-cmd/CONTEXT.md` — Boundaries] |
| `agentboard-action-worktree` | Action executor | None | Reads already-rendered `repo`, `root`, and `branch` strings and invokes Git. It does not depend on `Item` or `SourceConfig`. | **No source mapping; intentionally outside source boundary**. [`pkgs/crates/agentboard-action-worktree/src/lib.rs` — `create_worktree`; `pkgs/crates/agentboard-action-worktree/CONTEXT.md` — Boundaries] |

## Findings

1. **Common normalized shape does not imply common mapping implementation.** `FieldMap` centralizes four config field names, but source crates each contain their own dot-path walker and error vocabulary (`frontmatter mapping`, `jira mapping`, `github field_map`). No mapping function or adapter trait is exported by core. [`pkgs/crates/agentboard-core/src/model.rs` — `FieldMap`; all three source `src/lib.rs` files — `mapped_field`/`optional_mapped_field`]

2. **All implemented source kinds support mappings, but contract differs.** QMD maps parsed frontmatter and uses `map`; Jira/GitHub map provider JSON and use `field_map`. Jira additionally converts mapped `fields.*` paths into requested API fields; GitHub applies separate label/state status policy; QMD has no `status_map`. [`pkgs/crates/agentboard-core/src/model.rs` — `SourceKind`; source docs `pkgs/crates/agentboard-source-{qmd,jira,github}/src/docs.md` — “Field mapping”/“Identity, reference, and status”; corresponding normalization symbols]

3. **Mapping scope is deliberately narrow.** Users can select string-valued paths for `reference_id`, `title`, `status`, and `url`; they cannot configure adapter-owned `item.id`, `source_id`, `source_kind`, or `raw`, and there is no coercion or transform expression layer. Missing/non-string required values fail item normalization. [`pkgs/crates/agentboard-core/src/model.rs` — `FieldMap`, `Item`; source normalization and `mapped_field` symbols; `pkgs/crates/agentboard-core/CONTEXT.md` — identity definitions]

4. **Raw payload retention supplies escape hatch after normalization.** Each source stores its original/source-specific material in `Item.raw`; CLI persists that `Item` unchanged and exposes it to action templates. Action crates still see only rendered inputs, so use of raw data happens in CLI template expressions rather than executor mapping. [`pkgs/crates/agentboard-source-qmd/src/lib.rs`, `agentboard-source-jira/src/lib.rs`, `agentboard-source-github/src/lib.rs` — `Item { raw: ... }`; `apps/cli/src/store.rs` — `append_items`; `apps/cli/src/template.rs` — `render_action`]

5. **Current design uses closed explicit dispatch.** Adding a source requires a new `SourceKind` variant and CLI match arms, then adapter-local collection/normalization. Similar public function signatures are convention, not inheritance. GitHub Project mode is documented as deferred and is absent from `GithubSourceMode`, which currently contains only `Issue`. [`.memory/docs/adr/0008-use-one-github-source-with-explicit-modes.md` — Decision/Consequences; `pkgs/crates/agentboard-core/src/model.rs` — `GithubSourceMode`; `apps/cli/src/adapters.rs` — dispatch matches]

## Documentation/config mismatches

1. **GitHub source quick example is invalid as written.** `pkgs/crates/agentboard-source-github/src/docs.md` omits `status_map` from its opening TOML example. The core `Github` variant does not default `status_map`, and CLI tests prove omission fails TOML parsing while an explicit empty map fails semantic validation. [`pkgs/crates/agentboard-source-github/src/docs.md` — opening example; `pkgs/crates/agentboard-core/src/model.rs` — `SourceKind::Github`; `apps/cli/src/config.rs` — tests `github_status_map_must_be_explicit_in_config`, `github_status_map_rejects_empty_entries`]

2. **CLI docs misplace source-specific validation ownership.** `apps/cli/docs/sources.md` says source crates own source-specific validation, but `apps/cli/src/config.rs::validate_config` validates QMD collections/query/limit, Jira site/credentials/JQL/limit, and GitHub mode/query/helper/status map/limit before dispatch. Collection-time payload/path validation does remain in source crates. [`apps/cli/docs/sources.md` — “CLI-Owned Behavior”; `apps/cli/src/config.rs` — `validate_config`; source `mapped_field` functions]

3. **Context index omits an implemented crate.** `Cargo.toml`, CLI dependencies/dispatch, and a dedicated GitHub `CONTEXT.md` establish `agentboard-source-github`, but the context table in `CONTEXT-MAP.md` and current-context list in `.memory/docs/agents/domain.md` omit it. This does not change runtime mapping, but it can cause incomplete architecture audits. [`Cargo.toml` — workspace members; `apps/cli/Cargo.toml`; `apps/cli/src/adapters.rs`; `pkgs/crates/agentboard-source-github/CONTEXT.md`; `CONTEXT-MAP.md` — Contexts; `.memory/docs/agents/domain.md` — Current context docs]

4. **Naming is intentionally/non-uniformly documented, not inherited.** QMD docs and config use `map`; Jira/GitHub use `field_map`. Generic source docs acknowledge both names, so users cannot rely on one universal mapping table despite the shared `FieldMap` Rust type. [`apps/cli/docs/sources.md` — “Normalized Item Shape”; `pkgs/crates/agentboard-source-qmd/src/docs.md`; `pkgs/crates/agentboard-source-jira/src/docs.md`; `pkgs/crates/agentboard-source-github/src/docs.md`]

## Sources

- Kept: `Cargo.toml` and every `pkgs/crates/*/Cargo.toml` — exhaustive workspace/crate inventory and dependency direction.
- Kept: `pkgs/crates/agentboard-core/src/model.rs` — authoritative config and normalized model.
- Kept: every `pkgs/crates/agentboard-source-*/src/lib.rs` and source `src/docs.md` — implemented mapping/default/raw behavior and user-facing claims.
- Kept: every `pkgs/crates/agentboard-action-*/src/lib.rs`, `src/docs.md`, and `CONTEXT.md` — proves action executors are downstream consumers, not source mappers.
- Kept: `apps/cli/src/{config,adapters,runtime,store,template}.rs` and `apps/cli/docs/*.md` — complete parse, dispatch, persistence, rendering, and action-consumption flow.
- Kept: `.memory/docs/adr/0008-use-one-github-source-with-explicit-modes.md`, `CONTEXT-MAP.md`, and relevant `CONTEXT.md` files — accepted boundaries and planned GitHub mode.
- Dropped: external web sources — repository source and first-party docs fully answer implementation question; external material cannot establish local behavior.

## Gaps

- No runtime commands were available in this research pane, so conclusions come from exhaustive static inspection of all workspace crate manifests and Rust source files rather than executing `moon run agentboard:test` or regenerating `agentboard schema`.
- No generic source trait or mapping engine exists to test independently. A future adapter must be audited in its own crate and added to the explicit `SourceKind`/CLI dispatch chain.

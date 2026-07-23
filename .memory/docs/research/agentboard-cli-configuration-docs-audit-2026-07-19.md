# Research: AgentBoard CLI configuration documentation

Historical snapshot: parser/schema references below describe the pre-Registry configuration path. Current Workspace loading and schema composition use registered typed Source and Action definitions while preserving the same TOML syntax.

## Summary

AgentBoard's docs are usable for a happy-path Workspace: they establish TOML syntax, the nested Source/Action shape, named and explicit-path invocation, and several realistic examples. They are not a complete configuration reference: precedence and merge behavior are absent, defaults and validation are scattered, environment expansion of every rendered Action input is undisclosed, and the GitHub source page's primary example is invalid because it omits required `status_map`. [`apps/cli/docs/workspaces.md:5-77`](../../../apps/cli/docs/workspaces.md) [`pkgs/crates/agentboard-core/src/model.rs:7-105`](../../../pkgs/crates/agentboard-core/src/model.rs) [`pkgs/crates/agentboard-source-github/src/docs.md:5-19`](../../../pkgs/crates/agentboard-source-github/src/docs.md)

Overall assessment: **partial, with one correctness defect**. Core concepts are clear; users still need generated schema or source code to discover important behavior, and generated schema does not cover semantic validation performed by the CLI. [`apps/cli/docs/commands.md:98-106`](../../../apps/cli/docs/commands.md) [`apps/cli/src/config.rs:99-173`](../../../apps/cli/src/config.rs)

## Findings

1. **Coverage is uneven across required configuration topics** — syntax and examples are strongest; precedence/merging and overrides are weakest.

   | Topic | Rating | Evidence and assessment |
   | --- | --- | --- |
   | Syntax and shape | Good for common cases | Workspace, Source, Action, and `with` table nesting are shown repeatedly. There is no consolidated field/type/requiredness reference. [`apps/cli/docs/workspaces.md:35-66`](../../../apps/cli/docs/workspaces.md) [`apps/cli/docs/sources.md:11-27`](../../../apps/cli/docs/sources.md) |
   | Supported formats | Partial | Docs call a Workspace a TOML file, and implementation only deserializes TOML. Docs never explicitly say JSON/YAML are unsupported; JSON is schema/output only. [`apps/cli/docs/workspaces.md:5-12`](../../../apps/cli/docs/workspaces.md) [`apps/cli/src/config.rs:73-80`](../../../apps/cli/src/config.rs) [`apps/cli/docs/commands.md:98-106`](../../../apps/cli/docs/commands.md) |
   | Location/discovery | Partial | Named and path forms are documented. XDG/platform behavior, input classification, path expansion, and lack of project-directory auto-discovery are not fully explained. [`apps/cli/docs/workspaces.md:9-33`](../../../apps/cli/docs/workspaces.md) [`apps/cli/src/config.rs:12-39`](../../../apps/cli/src/config.rs) [`apps/cli/src/config.rs:67-97`](../../../apps/cli/src/config.rs) |
   | Precedence/merging | Missing | Implementation loads exactly one file and has no include, profile, overlay, or merge layer. Docs make no explicit no-merge statement. [`apps/cli/src/config.rs:67-97`](../../../apps/cli/src/config.rs) |
   | Defaults | Partial | Jira credential defaults are documented; source limits, empty maps/lists, optional Actions, and several normalization defaults are only implicit in examples or source code. [`pkgs/crates/agentboard-source-jira/src/docs.md:15-25`](../../../pkgs/crates/agentboard-source-jira/src/docs.md) [`pkgs/crates/agentboard-core/src/model.rs:24-89`](../../../pkgs/crates/agentboard-core/src/model.rs) |
   | Environment/CLI overrides | Poor | Jira credential environment variables and `$EDITOR` are documented. Action-input environment interpolation, config/data directory environment behavior, `NO_COLOR`, and absence of field-level CLI overrides are not gathered or clearly scoped. [`pkgs/crates/agentboard-source-jira/src/docs.md:15-41`](../../../pkgs/crates/agentboard-source-jira/src/docs.md) [`apps/cli/docs/commands.md:23-30`](../../../apps/cli/docs/commands.md) [`apps/cli/src/template.rs:12-38`](../../../apps/cli/src/template.rs) |
   | Validation/errors | Partial | A short rule list and `doctor` exist, but most source invariants, Action-input typing, schema limitations, and runtime mapping/credential errors are undisclosed. [`apps/cli/docs/workspaces.md:55-77`](../../../apps/cli/docs/workspaces.md) [`apps/cli/docs/commands.md:82-106`](../../../apps/cli/docs/commands.md) [`apps/cli/src/config.rs:99-190`](../../../apps/cli/src/config.rs) |
   | Examples | Mixed | Root quickstart and QMD/Jira/Action examples are useful. GitHub's lead example cannot pass current parsing/validation. [`README.md:37-81`](../../../README.md) [`pkgs/crates/agentboard-source-qmd/src/docs.md:5-37`](../../../pkgs/crates/agentboard-source-qmd/src/docs.md) [`pkgs/crates/agentboard-source-github/src/docs.md:5-19`](../../../pkgs/crates/agentboard-source-github/src/docs.md) |

2. **TOML is the sole implemented Workspace format, but exclusivity is only implicit** — `load_workspace_inner` reads one text file and calls `toml::from_str::<WorkspaceConfig>`; there is no JSON/YAML parser or format dispatch. A path containing `/` is treated as explicit regardless of extension, so TOML content can technically live in a non-`.toml` file when addressed as `./name`; a bare input is explicit only when it ends in `.toml`. [`apps/cli/src/config.rs:67-97`](../../../apps/cli/src/config.rs)

   Docs correctly call Workspaces TOML and demonstrate valid TOML table/array syntax. They should say “TOML only” and distinguish `agentboard schema`'s JSON output from accepted configuration formats. [`apps/cli/docs/workspaces.md:5-12`](../../../apps/cli/docs/workspaces.md) [`apps/cli/docs/workspaces.md:35-53`](../../../apps/cli/docs/workspaces.md) [`apps/cli/docs/commands.md:98-106`](../../../apps/cli/docs/commands.md)

3. **Documented file locations are directionally correct but omit exact discovery rules** — named Workspaces resolve to `<platform config dir>/agentboard/<name>.toml`; listing scans that one directory non-recursively, keeps regular files with exactly the `toml` extension, and sorts file stems. The command page mentions `~/.config/agentboard` “or the platform config directory,” while the Workspace page shows only the Linux-style path. [`apps/cli/src/config.rs:12-39`](../../../apps/cli/src/config.rs) [`apps/cli/src/config.rs:41-59`](../../../apps/cli/src/config.rs) [`apps/cli/src/config.rs:196-214`](../../../apps/cli/src/config.rs) [`apps/cli/docs/commands.md:7-15`](../../../apps/cli/docs/commands.md) [`apps/cli/docs/workspaces.md:9-25`](../../../apps/cli/docs/workspaces.md)

   Explicit Workspace inputs support leading `~/`, `$VAR`, and `${VAR}` expansion; explicit paths are canonicalized before their path-derived Workspace id is built. Neither behavior is documented. There is no upward search, current-project convention, or implicit default Workspace: every operational command requires a Workspace positional argument. [`apps/cli/src/config.rs:67-97`](../../../apps/cli/src/config.rs) [`apps/cli/src/config.rs:276-296`](../../../apps/cli/src/config.rs) [`apps/cli/src/cli.rs:40-78`](../../../apps/cli/src/cli.rs)

   Input classification creates a hidden edge case: `work.toml` means a relative explicit path, while `work` means a named Workspace; a bare non-`.toml` local filename is interpreted as a name unless prefixed with `./`. Docs show the two normal forms but do not explain this rule. [`apps/cli/docs/workspaces.md:13-25`](../../../apps/cli/docs/workspaces.md) [`apps/cli/src/config.rs:67-74`](../../../apps/cli/src/config.rs)

4. **No configuration merge or precedence system exists** — one input selects one file; that file is parsed into one `WorkspaceConfig`. No system/user/project layers, includes, profiles, or repeated-file merges are implemented. Therefore there is no file precedence to describe, but docs should explicitly state this because “named Workspace” and “explicit path” can otherwise be mistaken for layered config sources. [`apps/cli/src/config.rs:67-97`](../../../apps/cli/src/config.rs) [`pkgs/crates/agentboard-core/src/model.rs:7-20`](../../../pkgs/crates/agentboard-core/src/model.rs)

   Actual flow:

   ```text
   workspace argument
          |
          v
   classify named vs explicit path
          |
          v
   read one file -> TOML deserialize
          |                 |
          |                 +-- structural/type/unknown-field error -> stop
          v
   normal commands: semantic validation -> canonical path/id -> execute
   doctor:          canonical path/id -> aggregate semantic + environment checks
   ```

   Normal commands validate before execution. `doctor` deliberately parses without semantic validation, then reports semantic and environment checks together; structural TOML errors still prevent `doctor` from starting. [`apps/cli/src/config.rs:61-104`](../../../apps/cli/src/config.rs) [`apps/cli/src/cli.rs:127-175`](../../../apps/cli/src/cli.rs) [`apps/cli/src/store.rs:248-378`](../../../apps/cli/src/store.rs)

5. **Action-input environment expansion is important, implemented, and undocumented** — after MiniJinja rendering, every string under `[sources.actions.with]` is passed through `expand_vars`, which replaces leading `~/`, `$VAR`, and `${VAR}` from the current process environment. This applies to `cmd`, `cwd`, `repo`, `root`, `branch`, and arbitrary custom keys—not only filesystem paths. Expanded values also feed the rendered Action hash, so an environment-value change can make an Action eligible to run again. [`apps/cli/src/template.rs:12-38`](../../../apps/cli/src/template.rs) [`apps/cli/src/config.rs:276-296`](../../../apps/cli/src/config.rs) [`apps/cli/src/runtime.rs:205-239`](../../../apps/cli/src/runtime.rs)

   This behavior can surprise users: `cmd = "echo $HOME"` is expanded by AgentBoard before `sh -c`, even if shell quoting would otherwise defer or suppress expansion; unresolved variables remain literal. Action docs explain templates and execution but not this second expansion pass. [`pkgs/crates/agentboard-action-run-cmd/src/docs.md:5-32`](../../../pkgs/crates/agentboard-action-run-cmd/src/docs.md) [`apps/cli/src/template.rs:20-32`](../../../apps/cli/src/template.rs)

   Other environment behavior is fragmented:
   - Jira reads variables named by `email_env`/`token_env`, defaulting to `JIRA_EMAIL`/`JIRA_API_TOKEN`; configured `credentials.helper` takes precedence and bypasses those variables. This is documented accurately. [`pkgs/crates/agentboard-core/src/model.rs:33-50`](../../../pkgs/crates/agentboard-core/src/model.rs) [`pkgs/crates/agentboard-source-jira/src/lib.rs:130-152`](../../../pkgs/crates/agentboard-source-jira/src/lib.rs) [`pkgs/crates/agentboard-source-jira/src/docs.md:15-41`](../../../pkgs/crates/agentboard-source-jira/src/docs.md)
   - `$EDITOR` controls only `workspace edit`, and docs accurately explain parsing and final path argument. [`apps/cli/docs/commands.md:23-30`](../../../apps/cli/docs/commands.md) [`apps/cli/src/cli.rs:82-114`](../../../apps/cli/src/cli.rs)
   - Config/data roots come from platform directories; `NO_COLOR` affects automatic color. These are runtime environment controls, not Workspace-field overrides. [`apps/cli/src/config.rs:196-214`](../../../apps/cli/src/config.rs) [`apps/cli/src/output.rs:16-49`](../../../apps/cli/src/output.rs)
   - CLI has no `--set`, `--config`, profile, or source/action field override. Its flags control execution/output (`--dry-run`, `--interval`, verbosity, color, log file, JSON output), while the Workspace argument selects the entire file. [`apps/cli/src/cli.rs:15-78`](../../../apps/cli/src/cli.rs)

6. **Defaults are implemented consistently but not presented as a usable reference** — key defaults are:

   | Field/behavior | Implemented default | Documentation status |
   | --- | --- | --- |
   | `sources[].actions` | `[]` | Implied by empty Workspace initialization, not listed as a schema default. [`pkgs/crates/agentboard-core/src/model.rs:13-20`](../../../pkgs/crates/agentboard-core/src/model.rs) [`apps/cli/src/config.rs:50-59`](../../../apps/cli/src/config.rs) |
   | QMD/Jira/GitHub `limit` | `50` | Examples use 50, but source pages do not clearly label it as optional/default. [`pkgs/crates/agentboard-core/src/model.rs:24-60`](../../../pkgs/crates/agentboard-core/src/model.rs) [`pkgs/crates/agentboard-core/src/model.rs:79-81`](../../../pkgs/crates/agentboard-core/src/model.rs) |
   | QMD `map` | empty; adapter defaults to `id`, `title`, `status`, `url` | Mapping behavior is documented well. [`pkgs/crates/agentboard-core/src/model.rs:25-32`](../../../pkgs/crates/agentboard-core/src/model.rs) [`pkgs/crates/agentboard-source-qmd/src/lib.rs:39-68`](../../../pkgs/crates/agentboard-source-qmd/src/lib.rs) [`pkgs/crates/agentboard-source-qmd/src/docs.md:17-37`](../../../pkgs/crates/agentboard-source-qmd/src/docs.md) |
   | Jira credential variable names | `JIRA_EMAIL`, `JIRA_API_TOKEN` | Explicitly documented. [`pkgs/crates/agentboard-core/src/model.rs:33-40`](../../../pkgs/crates/agentboard-core/src/model.rs) [`pkgs/crates/agentboard-source-jira/src/docs.md:15-25`](../../../pkgs/crates/agentboard-source-jira/src/docs.md) |
   | Jira `credentials`, `fields`, `field_map`, `status_map` | none/empty | Field and status mapping fallbacks are explained; optionality/default values are not consolidated. [`pkgs/crates/agentboard-core/src/model.rs:33-50`](../../../pkgs/crates/agentboard-core/src/model.rs) [`pkgs/crates/agentboard-source-jira/src/docs.md:43-76`](../../../pkgs/crates/agentboard-source-jira/src/docs.md) |
   | GitHub `field_map`, `limit` | empty/50 | Behavior is described or exemplified, not identified as defaults. [`pkgs/crates/agentboard-core/src/model.rs:51-60`](../../../pkgs/crates/agentboard-core/src/model.rs) [`pkgs/crates/agentboard-source-github/src/docs.md:21-48`](../../../pkgs/crates/agentboard-source-github/src/docs.md) |
   | GitHub `mode`, `query`, `credentials`, `status_map` | no default; all required structurally, and `status_map` must be nonempty semantically | Requiredness is not stated, and lead example omits `status_map`. [`pkgs/crates/agentboard-core/src/model.rs:51-60`](../../../pkgs/crates/agentboard-core/src/model.rs) [`apps/cli/src/config.rs:143-169`](../../../apps/cli/src/config.rs) |
   | Action `with` | empty map structurally; built-ins require selected keys semantically | Docs show required keys but do not distinguish required from optional or explain string-only values. [`pkgs/crates/agentboard-core/src/model.rs:91-98`](../../../pkgs/crates/agentboard-core/src/model.rs) [`apps/cli/src/config.rs:176-190`](../../../apps/cli/src/config.rs) |

7. **Validation has three layers, while docs mostly describe one** — structural deserialization rejects missing required fields, wrong types, unsupported source variants, and unknown fields. `with` accepts arbitrary key names but every value must deserialize as a string because its type is `BTreeMap<String, String>`; the docs' “arbitrary keys” statement is correct but incomplete without the string-value constraint. [`pkgs/crates/agentboard-core/src/model.rs:7-98`](../../../pkgs/crates/agentboard-core/src/model.rs) [`apps/cli/docs/workspaces.md:55-55`](../../../apps/cli/docs/workspaces.md)

   CLI semantic validation additionally enforces:
   - nonempty, unique Source ids;
   - QMD: at least one collection, nonblank query, `limit > 0`;
   - Jira: nonblank site/JQL, nonblank helper or nonblank environment-variable names, `limit > 0`;
   - GitHub issue mode: nonblank query/helper, nonempty `status_map`, nonblank map keys/values, `limit > 0`;
   - known Action names and presence of built-in required input keys. [`apps/cli/src/config.rs:99-190`](../../../apps/cli/src/config.rs)

   Only Source-id rules, unknown Actions, and unknown fields appear in the Workspace page. Source pages show required-looking examples but do not enumerate these invariants or likely error messages. [`apps/cli/docs/workspaces.md:68-77`](../../../apps/cli/docs/workspaces.md) [`pkgs/crates/agentboard-source-qmd/src/docs.md:5-37`](../../../pkgs/crates/agentboard-source-qmd/src/docs.md) [`pkgs/crates/agentboard-source-jira/src/docs.md:5-76`](../../../pkgs/crates/agentboard-source-jira/src/docs.md)

   Runtime validation adds credential-helper failures, missing environment variables, API/command failures, duplicate Item identities, and mapping paths that must resolve to strings. Source docs cover some mapping requirements and credential shapes, but no error-oriented examples or troubleshooting map connects failures to fixes. [`pkgs/crates/agentboard-source-qmd/src/lib.rs:17-37`](../../../pkgs/crates/agentboard-source-qmd/src/lib.rs) [`pkgs/crates/agentboard-source-qmd/src/lib.rs:70-145`](../../../pkgs/crates/agentboard-source-qmd/src/lib.rs) [`pkgs/crates/agentboard-source-jira/src/lib.rs:41-76`](../../../pkgs/crates/agentboard-source-jira/src/lib.rs) [`pkgs/crates/agentboard-source-jira/src/lib.rs:130-203`](../../../pkgs/crates/agentboard-source-jira/src/lib.rs) [`pkgs/crates/agentboard-source-github/src/lib.rs:41-83`](../../../pkgs/crates/agentboard-source-github/src/lib.rs) [`pkgs/crates/agentboard-source-github/src/lib.rs:188-211`](../../../pkgs/crates/agentboard-source-github/src/lib.rs)

   `doctor` is accurately advertised as checking config, Store writability, commands, and Source reachability. Missing nuance: it aggregates semantic/environment failures, but it cannot report through malformed TOML because deserialization happens before `doctor`. [`apps/cli/docs/commands.md:82-96`](../../../apps/cli/docs/commands.md) [`apps/cli/src/config.rs:61-80`](../../../apps/cli/src/config.rs) [`apps/cli/src/store.rs:248-378`](../../../apps/cli/src/store.rs)

8. **Generated JSON Schema is useful but cannot validate all documented CLI rules** — `agentboard schema` derives from `WorkspaceConfig`, so it represents structural fields, types, variants, Serde defaults, and unknown-field policy. It does not encode `validate_config`'s duplicate detection, trimmed nonempty strings, nonempty QMD collections/GitHub status maps, positive limits, Action-name allowlist, or Action-specific required inputs. Docs place schema immediately after “CLI validation rules” without warning about this boundary, creating an over-validation risk for editor users. [`apps/cli/src/cli.rs:163-171`](../../../apps/cli/src/cli.rs) [`pkgs/crates/agentboard-core/src/model.rs:7-98`](../../../pkgs/crates/agentboard-core/src/model.rs) [`apps/cli/src/config.rs:99-190`](../../../apps/cli/src/config.rs) [`apps/cli/docs/workspaces.md:68-77`](../../../apps/cli/docs/workspaces.md)

9. **One first-party example is definitively stale/incorrect** — the GitHub source page's lead TOML includes `kind`, `mode`, `query`, `limit`, and credentials, but omits `status_map`. The model requires `status_map` during TOML deserialization, and a unit test explicitly verifies omission fails; even `{}` then fails semantic validation. [`pkgs/crates/agentboard-source-github/src/docs.md:5-19`](../../../pkgs/crates/agentboard-source-github/src/docs.md) [`pkgs/crates/agentboard-core/src/model.rs:51-60`](../../../pkgs/crates/agentboard-core/src/model.rs) [`apps/cli/src/config.rs:376-407`](../../../apps/cli/src/config.rs)

   Root README's GitHub quickstart does include a nonempty inline `status_map`, so it matches current implementation. QMD/Jira examples match their structural shape, and Action examples provide the required built-in keys. [`README.md:37-81`](../../../README.md) [`pkgs/crates/agentboard-source-qmd/src/docs.md:5-37`](../../../pkgs/crates/agentboard-source-qmd/src/docs.md) [`pkgs/crates/agentboard-source-jira/src/docs.md:5-76`](../../../pkgs/crates/agentboard-source-jira/src/docs.md) [`pkgs/crates/agentboard-action-run-cmd/src/docs.md:5-32`](../../../pkgs/crates/agentboard-action-run-cmd/src/docs.md) [`pkgs/crates/agentboard-action-worktree/src/docs.md:5-24`](../../../pkgs/crates/agentboard-action-worktree/src/docs.md)

10. **Highest-value documentation fixes are small and concrete** — priority order:

    1. Fix GitHub lead example by adding nonempty `status_map`; mark `mode`, `query`, `credentials`, and `status_map` required. [`pkgs/crates/agentboard-source-github/src/docs.md:5-48`](../../../pkgs/crates/agentboard-source-github/src/docs.md)
    2. Add one Workspace “loading and precedence” section: TOML only, exactly one file, no merge/includes, named-vs-path classification, platform/XDG location, and path expansion. [`apps/cli/src/config.rs:12-39`](../../../apps/cli/src/config.rs) [`apps/cli/src/config.rs:67-97`](../../../apps/cli/src/config.rs) [`apps/cli/src/config.rs:196-214`](../../../apps/cli/src/config.rs) [`apps/cli/src/config.rs:276-296`](../../../apps/cli/src/config.rs)
    3. Add a compact defaults/required-fields table generated or manually checked against `model.rs`; distinguish structural schema validation from semantic `doctor` checks. [`pkgs/crates/agentboard-core/src/model.rs:7-98`](../../../pkgs/crates/agentboard-core/src/model.rs) [`apps/cli/src/config.rs:99-190`](../../../apps/cli/src/config.rs)
    4. Document Action input processing order: MiniJinja render → environment/home expansion → hash → execute, including retry implications. [`apps/cli/src/template.rs:12-38`](../../../apps/cli/src/template.rs) [`apps/cli/src/runtime.rs:205-239`](../../../apps/cli/src/runtime.rs)
    5. Add one invalid-config/troubleshooting example and state that `with` values are strings. [`pkgs/crates/agentboard-core/src/model.rs:91-98`](../../../pkgs/crates/agentboard-core/src/model.rs) [`apps/cli/src/store.rs:248-378`](../../../apps/cli/src/store.rs)

## Sources

- Kept: `README.md` — first-party quickstart and complete GitHub Workspace example.
- Kept: `apps/cli/docs/{workspaces,commands,sources,actions,store}.md` — public CLI documentation under audit.
- Kept: `apps/cli/src/{cli,config,store,template,runtime,adapters}.rs` — implemented argument parsing, file loading, validation, environment expansion, `doctor`, and execution flow.
- Kept: `pkgs/crates/agentboard-core/src/model.rs` — authoritative typed configuration model and Serde/Schemars defaults.
- Kept: `pkgs/crates/agentboard-source-{qmd,jira,github}/src/{docs.md,lib.rs}` — first-party source configuration docs and runtime behavior.
- Kept: `pkgs/crates/agentboard-action-{run-cmd,worktree}/src/{docs.md,lib.rs}` — first-party Action input docs and executors.
- Kept: inline Rust tests and `apps/cli/test/test_helper.bash` — evidence for required GitHub `status_map`, defaults, and XDG-based integration setup. [`apps/cli/src/config.rs:315-430`](../../../apps/cli/src/config.rs) [`apps/cli/test/test_helper.bash:1-18`](../../../apps/cli/test/test_helper.bash)
- Dropped: external search results for other projects named “AgentBoard” — name collision, no relevance or authority for this repository.
- Dropped: dependency documentation — repository code is sufficient to establish observed config behavior; no dependency-specific ambiguity required an external source.

## Gaps

- This is a static repository audit. Generated `--help`, emitted JSON Schema, built documentation site, and released binary were not executed/compared; source derives those surfaces, but generated-output regressions remain possible. [`apps/cli/src/cli.rs:11-80`](../../../apps/cli/src/cli.rs) [`apps/cli/src/cli.rs:163-171`](../../../apps/cli/src/cli.rs)
- No research/notes placement convention is declared in root project instructions, `CONTEXT-MAP.md`, or `.memory/docs/agents/domain.md`. This report therefore uses `.memory/docs/research/`, adjacent to existing durable internal engineering documentation; only this report was created. [`CONTEXT-MAP.md:1-42`](../../../CONTEXT-MAP.md) [`domain.md:1-39`](../agents/domain.md)
- Released-site freshness was not assessed. If release correctness matters, next step is compare current repository pages and generated schema/help with the latest published `agentboard-v*` binary and site deployment.

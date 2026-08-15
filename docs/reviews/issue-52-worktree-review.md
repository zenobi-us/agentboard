# Review: AgentBoard issue #52

## Verdict

FAILURE

## Scope

- Issue: GitHub issue #52, `feat: execute resolved Sources and Actions in Bun`.
- Issue URL: <https://github.com/zenobi-us/agentboard/issues/52>.
- Worktree: `/run/media/zenobius/Store/Projects/Mine/Github/AgentBoard.worktrees/issue-52-bun-runtime`.
- Source branch: `issue-52-bun-runtime`.
- Pull request: none.
- Base branch: `origin/main`.
- Base commit: `90cfa452b8ba71e012fcc12c8a4e97e32435246d`.
- Source commit: `17bee5fdd15e3781b2caad1bf5d89a966ddf21ea`.
- Reviewed staged diff SHA-256: `9f0cdb9ed7ad3f008173348b6b91a120a4a57d61b5e76ac53f7a81d009393066`.
- Effective diff: `git diff origin/main --`.
- Active `ALIGNMENT_ROOT`: `/run/media/zenobius/Store/Projects/Mine/Github/AgentBoard.worktrees/issue-52-bun-runtime`.
- Alignment storage: `repository`.
- Timestamp: `2026-08-14T10:57:48+09:30`.

Herdr and Worktrunk resolved this worktree and branch. The branch is six commits ahead of `origin/main`.

The `/eng-context report` command produced no report. No `eng-context` executable is installed.

## Standards

### Blocking findings

1. `pkgs/crates/agentboard-core/src/config.test.ts:13` does not type-check.
   - The Source Plugin runtime returns configuration data instead of a `SourceRuntime`.
   - `agentboard-core:ts-typecheck` fails with `TS2741`.

2. `apps/cli/src/services/plugins.ts:249-262` duplicates `isPluginDescriptor` from `pkgs/crates/agentboard-core/src/config.ts:199-213`.
   - `CONTEXT-MAP.md` and `pkgs/crates/agentboard-core/CONTEXT.md` assign registration contracts and small shared helpers to Core.
   - The CLI must import the Core helper instead of owning a second copy.

### Non-blocking findings

- `apps/cli/src/services/sources.ts:18-31` accepts `workspaceId` and discards it.
- `PluginKind` and `SourceCollection` have no current TypeScript consumers.

## Spec

### Blocking findings

1. Action runtime factory errors do not fail Workspace loading.
   - Issue #52 requires configuration and runtime factory errors to fail Workspace loading.
   - `apps/cli/src/services/config/workspace.ts:105-120` runs only Action runtime preparation.
   - `apps/cli/src/services/runtime.ts:207` runs the rendered-input factory during a Run.
   - `apps/cli/src/services/runtime.ts:231-232` makes this error Run-wide, not a Workspace loading error.
   - `Promise.all` then rejects before sibling Source pipelines stop.
   - `apps/cli/src/services/runtime.ts:114-115` releases `run.lock` while sibling work can remain active.

2. Dynamic named Action aliases reject valid templates.
   - ADR `0011-expose-preceding-named-action-inputs.md` requires valid preceding named Action references to render.
   - `apps/cli/src/services/template.ts:124-135` does not model dynamic bracket aliases.
   - This valid template throws `undefined value: actions.inputs.root`:

```jinja
{% set key = "worktree" %}
{% set prior = actions[key] %}
{{ prior.inputs.root }}
```

   - Dynamic missing references in unused assignments can also pass without an error.
   - The regex parser does not enforce one reliable named Action reference contract.

### Resolved prior findings

- Source pipelines now start concurrently.
- Raw blocks and comments do not cause strict-reference failures.
- Static named Action aliases now fail on missing fields.

## Validation

- `moon run agentboard:ts-test agentboard:ts-typecheck --force`: PASS. 61 tests passed. The CLI type check passed.
- `moon run agentboard-core:ts-test agentboard-core:ts-typecheck --force`: FAILURE. Eight tests passed. TypeScript failed with `TS2741` at `src/config.test.ts:13`.
- `git diff --check origin/main`: PASS.
- Dynamic alias reproduction: FAILURE. A valid alias threw `undefined value: actions.inputs.root`.
- Concurrent fatal-error reproduction: FAILURE. The Run returned and released `run.lock` before a sibling Source finished.

## Review axes

- Standards: two blocking findings and two non-blocking findings.
- Spec: two blocking findings.

## Fix result

**Status:** FIXED, NOT RE-REVIEWED

Timestamp: `2026-08-14T11:13:24+09:30`.

The fix pass made these changes:

- The Core Source Plugin fixture now returns a `SourceRuntime`.
- The CLI now uses the exported Core `isPluginDescriptor()` helper.
- Workspace loading still rejects Action runtime preparation errors.
- A fatal rendered Action factory error now waits for every Source pipeline before `run.lock` is released.
- A literal dynamic named Action alias now renders a preceding named Action.
- Missing and forward named Action aliases still fail.
- Raw blocks, comments, and unrelated missing Item fields keep their prior behavior.

Focused tests prove that Source pipelines start concurrently. They also prove that a fatal Action factory error cannot release `run.lock` while a sibling Source pipeline remains active.

The focused runtime command passed four tests. These tests cover concurrent Sources, lock lifetime, dynamic aliases, and invalid named Action references.

### Fix validation

Command:

```bash
moon run agentboard:ts-test agentboard:ts-typecheck agentboard-core:ts-test agentboard-core:ts-typecheck --force && git diff --check origin/main
```

Result:

- `agentboard:ts-test`: PASS, 63 tests.
- `agentboard:ts-typecheck`: PASS.
- `agentboard-core:ts-test`: PASS, 9 tests.
- `agentboard-core:ts-typecheck`: PASS.
- `git diff --check origin/main`: PASS.

The verdict remains `FAILURE`. A new review must set the next verdict.

## Fix workflow

- Fixer agent: `issue52-fixer`.
- Herdr pane: `wS:p12`.
- Source branch: `issue-52-bun-runtime`.
- Source worktree: `/run/media/zenobius/Store/Projects/Mine/Github/AgentBoard.worktrees/issue-52-bun-runtime`.
- Handoff: `/tmp/issue-52-fix-handoff.md`.
- Active `ALIGNMENT_ROOT`: `/run/media/zenobius/Store/Projects/Mine/Github/AgentBoard.worktrees/issue-52-bun-runtime`.
- Alignment storage: `repository`.
- Engineering skill: Matt Pocock `tdd`.
- Expected validation: `moon run agentboard:ts-test agentboard:ts-typecheck agentboard-core:ts-test agentboard-core:ts-typecheck --force && git diff --check origin/main`.
- Started: `2026-08-14T11:03:39+09:30`.

## Post-fix re-review

**Verdict:** FAILURE

- Reviewed source commit: `17bee5fdd15e3781b2caad1bf5d89a966ddf21ea` plus staged and unstaged worktree changes.
- Reviewed diff SHA-256: `2d1a5adee7a018b6fd4ff2ee974b2d276be00f153c172be38c9279dd05c05eba`.
- Timestamp: `2026-08-15T12:12:57+09:30`.

### Blocking findings

1. Action runtime factory errors still occur during a Run.
   - Files: `apps/cli/src/services/config/workspace.ts`, `apps/cli/src/services/actions.ts`, `apps/cli/src/services/runtime.ts`, `pkgs/crates/agentboard-core/src/config.ts`.
   - Issue #52 requires configuration and runtime factory errors to fail Workspace loading.
   - `prepared.create(inputs)` still runs after Action input rendering during `runWorkspace()`.
   - The Workspace loads successfully before this factory can fail.

2. Dynamic named Action aliases remain partial.
   - Files: `apps/cli/src/services/template.ts`, `apps/cli/src/services/runtime.test.ts`.
   - Literal string assignments now work.
   - Valid keys from a context value or string expression still fail with `undefined value: actions.key`.
   - Missing and forward named Action references must still fail.

3. The canonical TOML files break the retained Rust CLI.
   - Files: `.agentboard.toml`, `apps/demo/.agentboard.toml`.
   - Issue #52 prohibits migration or removal of the existing Rust implementation.
   - The files replace `source.kind` with `source.uses`.
   - The Rust loader still requires `source.kind` and cannot load either canonical file.

### Validation

- `moon run agentboard:ts-test agentboard:ts-typecheck agentboard-core:ts-test agentboard-core:ts-typecheck --force && git diff --check origin/main`: PASS. 63 CLI tests and nine Core tests passed.
- `bun /tmp/issue52-concurrent-fatal-repro.ts`: PASS. The Run kept `run.lock` until the sibling Source finished.
- Computed dynamic alias reproduction: FAILURE. Valid context and concatenated keys failed with `undefined value: actions.key`.
- `moon run agentboard:build --force && ./target/debug/agentboard doctor .agentboard.toml`: FAILURE. The Rust loader reported `missing field 'kind'`.

## Second fix workflow

- Fixer agent: `issue52-fixer-2`.
- Herdr pane: `wS:p16`.
- Source branch: `issue-52-bun-runtime`.
- Source worktree: `/run/media/zenobius/Store/Projects/Mine/Github/AgentBoard.worktrees/issue-52-bun-runtime`.
- Handoff: `/tmp/issue-52-fix-handoff.md`.
- Active `ALIGNMENT_ROOT`: `/run/media/zenobius/Store/Projects/Mine/Github/AgentBoard.worktrees/issue-52-bun-runtime`.
- Alignment storage: `repository`.
- Engineering skill: Matt Pocock `diagnosing-bugs`.
- Expected validation: `moon run agentboard:ts-test agentboard:ts-typecheck agentboard-core:ts-test agentboard-core:ts-typecheck --force && moon run agentboard:build --force && test -z "$(git diff origin/main -- .agentboard.toml apps/demo/.agentboard.toml)" && git diff --check origin/main`.
- Started: `2026-08-15T12:15:00+09:30`.

## Second fix result

**Status:** PARTIAL FIX, BLOCKED BY CONTRACT CONTRADICTION

Timestamp: `2026-08-15T12:23:34+09:30`.

The verdict remains `FAILURE`. A new review must set the next verdict.

### Action runtime factory contradiction

The current interface cannot satisfy both ticket requirements.

- `PreparedActionRuntime.create(inputs)` is the per-Rendered-Action runtime factory.
- Rendered Action inputs depend on each Item.
- Items do not exist until Source collection runs.
- Source collection occurs after Workspace loading.
- Workspace loading cannot call `create(inputs)` without fake inputs.
- The handoff prohibits fake inputs.
- Moving `create(inputs)` to Workspace loading would also remove one runtime per Rendered Action.

No Action runtime interface change was made. The red reproduction remains:

```bash
bun test /tmp/issue52-action-factory-repro.test.ts
```

Result: FAILURE. `loadExecutableWorkspace()` resolved instead of rejecting with `Action runtime factory failed: creation exploded`.

The current interface needs one contract decision. Either the per-Rendered-Action operation is not a Workspace-loading factory, or it must not require rendered inputs.

### Fixed findings

- Computed named Action keys now use MiniJinja expression evaluation.
- Context-driven named Action keys now resolve from the template context.
- Computed missing and forward named Action references still fail.
- Raw blocks, comments, and unrelated missing Item fields keep their prior behavior.
- `.agentboard.toml` and `apps/demo/.agentboard.toml` now match `origin/main`.
- The retained Rust CLI loads and checks the canonical Workspace.

### Focused validation

Command:

```bash
bun test apps/cli/src/services/runtime.test.ts -t 'starts all Source pipelines|keeps run.lock until every Source pipeline stops|dynamic aliases|missing and forward named Action|unrelated missing Item'
```

Result: PASS. Five tests passed. The command made 35 assertions.

Command:

```bash
bun test /tmp/issue52-dynamic-alias-repro.test.ts
```

Result: PASS. Two tests passed.

Command:

```bash
./target/debug/agentboard doctor .agentboard.toml
```

Result: PASS. The Rust CLI reported `ok config`, `ok store`, both Sources reachable, and all Action checks passed.

### Full validation

Command:

```bash
moon run agentboard:ts-test agentboard:ts-typecheck agentboard-core:ts-test agentboard-core:ts-typecheck --force \
  && moon run agentboard:build --force \
  && test -z "$(git diff origin/main -- .agentboard.toml apps/demo/.agentboard.toml)" \
  && git diff --check origin/main
```

Result: PASS.

- `agentboard:ts-test`: PASS, 63 tests and 175 assertions.
- `agentboard:ts-typecheck`: PASS.
- `agentboard-core:ts-test`: PASS, nine tests and 18 assertions.
- `agentboard-core:ts-typecheck`: PASS.
- `agentboard:build`: PASS.
- Canonical TOML diff check: PASS.
- `git diff --check origin/main`: PASS.

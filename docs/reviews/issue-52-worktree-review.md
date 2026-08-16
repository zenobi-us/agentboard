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
- Source commit: `96236135e43f0e792b3df338f72489c557df48ed`.
- Effective diff: tracked worktree changes from `git diff origin/main --`, plus all untracked files present before this artifact update.
- Active `ALIGNMENT_ROOT`: `/run/media/zenobius/Store/Projects/Mine/Github/AgentBoard.worktrees/issue-52-bun-runtime`.
- Alignment storage: `repository`.
- Timestamp: `2026-08-15T21:40:40+09:30`.

Herdr and Worktrunk resolved the worktree and branch. The branch is eight commits ahead of `origin/main`. The branch has no pull request.

The `eng-context report` command was unavailable because no `eng-context` executable is installed.

## Standards

### Blocking findings

1. **Watch Mode can exit with status 0 after cancellation.**
   - Files: `apps/cli/src/services/runtime.ts:117-145`, `apps/cli/src/cli/run.ts:55-62`.
   - If cancellation occurs during the interval, `watchWorkspace()` returns the previous successful result without `cancelled: true`.
   - `runExitStatus()` then returns `0`.
   - `waitForNextRun()` can also miss an abort between the guard and event-listener registration.
   - ADR 0012 requires cancellation to exit with status `130`.

2. **Cancellation can start new Source work.**
   - File: `apps/cli/src/services/runtime.ts:177-180`.
   - Cancellation can occur while the CLI writes the `collecting` status.
   - The CLI then calls `collectSource()` without another cancellation check.
   - ADR 0012 requires the first Ctrl-C to stop new work.

3. **Executable configuration can bypass external Plugin Package rules.**
   - File: `apps/cli/src/services/config/workspace.ts:347-372`.
   - An external package without the `agentboard-package` keyword can be treated as an inline Plugin.
   - A marked package that exports multiple Plugins can also supply one selected descriptor.
   - ADR 0013 requires the keyword outside project code and one Plugin per package.
   - This also creates incorrect Plugin and Store identities.

### Resolved findings

- Package discovery now requires one default Plugin Descriptor export.
- A focused test rejects a package that exports its only Plugin Descriptor as a named export.
- Package discovery still rejects packages that provide multiple Plugin Descriptors.

### Non-blocking findings

None.

## Spec

### Blocking findings

1. **Source IDs permit Store path traversal.**
   - Files: `apps/cli/src/services/config/workspace.ts:328,411`, `apps/cli/src/services/store.ts:64`.
   - Workspace loading accepts any nonempty Source ID.
   - An ID such as `../../target` escapes the Workspace Store when the CLI writes Source Collection Status.
   - Configuration validation must reject path components, or Store paths must encode the Source ID.

2. **JavaScript Action Plugins can retain the legacy `runtime` field.**
   - File: `pkgs/crates/agentboard-core/src/config.ts:212-220,303-309`.
   - `definePlugin()` and `isPluginDescriptor()` accept an Action with both `prepare` and `runtime`.
   - Issue #52 requires Action Plugins to expose `prepare` instead of `runtime`.
   - Runtime validation must reject `runtime` on Actions and `prepare` on Sources.

## Validation

Command:

```bash
moon run agentboard:ts-test agentboard:ts-typecheck agentboard-core:ts-test agentboard-core:ts-typecheck --force && git diff --check origin/main --
```

Result: PASS.

- `agentboard:ts-test`: 64 tests passed.
- `agentboard:ts-typecheck`: passed.
- `agentboard-core:ts-test`: 10 tests passed.
- `agentboard-core:ts-typecheck`: passed.
- `git diff --check origin/main --`: passed.

The validation command does not cover the five remaining blocking findings.

Focused fix validation:

```bash
bun test apps/cli/src/services/plugins.test.ts -t 'rejects a package without a default Plugin Descriptor export|rejects a package that exports more than one Plugin Descriptor' && moon run agentboard:ts-typecheck --force && git diff --check origin/main --
```

Result: PASS. Two focused tests and the CLI type check passed.

## Review axes

- Standards: three blocking findings and one resolved finding.
- Spec: two blocking findings.

## Next step

Run `worktree-fix` with only the five remaining blocking findings above. Use the validation command from this review after the fixes.

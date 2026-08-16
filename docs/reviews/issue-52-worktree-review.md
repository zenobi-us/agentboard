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
- Source commit: `5486f428a88058b606ba59ac19e92637f7fd2ba2`.
- Effective diff: `git diff origin/main --`.
- Active `ALIGNMENT_ROOT`: `/run/media/zenobius/Store/Projects/Mine/Github/AgentBoard.worktrees/issue-52-bun-runtime`.
- Alignment storage: `repository`.
- Timestamp: `2026-08-16T22:02:07+09:30`.

Herdr and Worktrunk resolved the worktree and branch. The branch is nine commits ahead of `origin/main`. No pull request exists.

The `eng-context report` command was unavailable because no `eng-context` executable is installed.

## Standards

### Blocking findings

1. **Watch Mode can exit with status 0 after cancellation.**
   - Files: `apps/cli/src/services/runtime.ts:116-161`, `apps/cli/src/cli/run.ts:23-24,62`.
   - Cancellation during the wait interval returns the previous successful result without `cancelled: true`.
   - `runExitStatus()` then returns `0` instead of `130`.
   - The wait can also miss an abort between its initial check and listener registration.
   - This violates ADR 0012.

2. **Cancellation can start new runtime or Source work.**
   - Files: `apps/cli/src/services/runtime.ts:176-179`, `apps/cli/src/services/config/workspace.ts:112-113,141-142,199-200,231-244`.
   - Cancellation can occur during an awaited status write or Action runtime creation.
   - The code can then start Source collection or Source runtime creation without another cancellation check.
   - ADR 0012 requires cancellation to stop new work.

3. **Executable configuration can bypass Plugin Package rules.**
   - File: `apps/cli/src/services/config/workspace.ts:351-376`.
   - An unmarked external package can be treated as an inline Plugin.
   - Executable configuration can also avoid the one-Plugin-per-package check used by package loading.
   - This violates ADR 0013 and the Plugin-backed Workspace plan.
   - The bypass can produce an incorrect inline Plugin and Store identity.

4. **Action Results can overwrite authoritative Store metadata.**
   - Files: `apps/cli/src/services/runtime.ts:266-272`, `apps/cli/src/services/actions.ts:58-66`.
   - `isActionResult()` accepts extra fields.
   - The `...result` spread occurs after `source_id`, `uses`, and `rendered_action_hash`.
   - A Plugin can replace authoritative attempt metadata and break success lookup.

### Non-blocking findings

None.

## Spec

### Blocking findings

1. **Watch cancellation does not preserve status 130.**
   - Files: `apps/cli/src/services/runtime.ts:127-161`, `apps/cli/src/cli/run.ts:23-24,62`.
   - Issue #52 requires preserved cancellation behavior.
   - Cancellation during the wait interval can return a successful result and status `0`.

2. **Cancellation can start Source work.**
   - File: `apps/cli/src/services/runtime.ts:176-179`.
   - Issue #52 requires preserved cancellation behavior.
   - Cancellation during the status write can still start provider collection.

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

The validation command does not cover the four unique blocking findings.

## Review axes

- Standards: four blocking findings.
- Spec: two blocking findings. Both cancellation findings overlap the Standards findings.

## Next step

Run `worktree-fix` with the four unique blocking findings above. Use the validation command from this review after the fixes.

## Fix workflow

- Status: started.
- Fixer agent: `issue52-fixer`.
- Herdr pane: `wS:p1J`.
- Source branch: `issue-52-bun-runtime`.
- Source worktree: `/run/media/zenobius/Store/Projects/Mine/Github/AgentBoard.worktrees/issue-52-bun-runtime`.
- Handoff: `/tmp/issue-52-fix-handoff.md`.
- Active `ALIGNMENT_ROOT`: `/run/media/zenobius/Store/Projects/Mine/Github/AgentBoard.worktrees/issue-52-bun-runtime`.
- Alignment storage: `repository`.
- Engineering skill: Matt Pocock `diagnosing-bugs`.
- Validation: `moon run agentboard:ts-test agentboard:ts-typecheck agentboard-core:ts-test agentboard-core:ts-typecheck --force && git diff --check origin/main --`.

## Fix result

- Status: fixed, not re-reviewed.
- Completed: `2026-08-16T22:38:00+09:30`.
- Watch cancellation now returns `cancelled: true` and exit status `130`.
- Cancellation now stops new Source and runtime work.
- Executable Plugins now enforce package markers and package identity.
- Action Results cannot overwrite authoritative Store metadata.
- Regression tests cover all four review blockers.
- Validation passed with 68 CLI tests, 10 Core tests, both TypeScript checks, and `git diff --check origin/main --`.
- The review verdict remains `FAILURE` until a new review replaces it.

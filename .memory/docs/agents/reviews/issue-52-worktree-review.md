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
- Source commit: `2b864eba7ea895f5aecba3801063bebe1204e086` plus current uncommitted fix changes.
- Effective diff: `git diff origin/main --`.
- Active `ALIGNMENT_ROOT`: `/run/media/zenobius/Store/Projects/Mine/Github/AgentBoard.worktrees/issue-52-bun-runtime`.
- Alignment storage: `repository`.
- Timestamp: `2026-08-17T15:38:55+09:30`.

`HERDR_ENV=1`. Herdr resolved the worktree. Worktrunk resolved the branch and base. No pull request exists. `eng-context report` is unavailable because no `eng-context` executable is installed.

## Findings

### Blocking

1. **Built-in Actions ignore rendered inputs.**
   - Files: `pkgs/crates/agentboard-action-run-cmd/src/runtime.ts:26-44`, `pkgs/crates/agentboard-action-worktree/src/runtime.ts:34-49`.
   - The runtimes close over factory-time configuration. They do not use `context.inputs`.
   - Rendered templates cannot change `cmd`, `cwd`, `repo`, `root`, or `branch`.
   - This violates the issue requirement that Action execution receives and uses rendered inputs.

2. **Built-in Source behavior regressed.**
   - Files: `pkgs/crates/agentboard-source-jira/src/runtime.ts:24-32`, `pkgs/crates/agentboard-source-github/src/runtime.ts:16-26`, `pkgs/crates/agentboard-source-qmd/src/runtime.ts:34-49`.
   - Jira and GitHub perform one request and do not paginate to the configured limit.
   - Jira, GitHub, and QMD do not reject duplicate Item identities.
   - The retained Rust implementations provide these behaviors.
   - This violates the requirement to preserve Source behavior.

3. **Built-in subprocess cancellation does not terminate process groups.**
   - Files: `pkgs/crates/agentboard-source-qmd/src/runtime.ts:22-24`, `pkgs/crates/agentboard-source-jira/src/runtime.ts:11-15`, `pkgs/crates/agentboard-source-github/src/runtime.ts:4-8`, `pkgs/crates/agentboard-action-run-cmd/src/runtime.ts:4-12`, `pkgs/crates/agentboard-action-worktree/src/runtime.ts:4-15`.
   - Cancellation calls `child.kill()` on the direct child only.
   - Descendant processes can remain alive.
   - ADR 0012 requires built-ins to terminate owned requests and process groups.

4. **Cancellation can start Action work after the cancellation point.**
   - File: `apps/cli/src/services/runtime.ts:197-214`.
   - `runSource()` does not check cancellation after the awaited `complete` status write before it calls `runActions()`.
   - Cancellation during that status write can start Action rendering or execution.
   - This violates ADR 0012.

5. **Run command health checks do not preserve timeout behavior.**
   - File: `pkgs/crates/agentboard-action-run-cmd/src/runtime.ts:33-40`.
   - The Bun implementation sleeps without cancellation or deadline control and drops final probe output on timeout.
   - The retained Rust implementation terminates timed-out probes and preserves launch and probe output.
   - This violates preserved Action behavior.

6. **Built-in configuration validation accepts invalid values.**
   - Files: `pkgs/crates/agentboard-source-qmd/src/config.ts:6-10`, `pkgs/crates/agentboard-source-jira/src/config.ts:12-25`, `pkgs/crates/agentboard-source-github/src/config.ts:12-18`.
   - The TypeBox schemas accept empty required strings and zero limits that the retained Rust builders reject.
   - Invalid configuration can reach runtime execution instead of failing Workspace loading.

7. **Jira and GitHub helpers can spawn after cancellation.**
   - Files: `pkgs/crates/agentboard-source-jira/src/runtime.ts:11-13`, `pkgs/crates/agentboard-source-github/src/runtime.ts:4-6`.
   - Both helpers call `Bun.spawn()` before checking `signal.aborted`.
   - Cancellation can occur before helper entry, so new Source work can start after cancellation.

8. **QMD health checks ignore cancellation after process creation.**
   - File: `pkgs/crates/agentboard-source-qmd/src/runtime.ts:54-58`.
   - The health check checks cancellation before `Bun.spawn()` only.
   - It does not stop or re-check the process after cancellation.

## Validation

- `moon run agentboard:ts-test agentboard:ts-typecheck agentboard-core:ts-test agentboard-core:ts-typecheck --force && git diff --check origin/main --`: PASS.
  - CLI TypeScript tests: 68 passed.
  - Core TypeScript tests: 10 passed.
  - CLI and Core TypeScript checks passed.
  - `git diff --check origin/main --` passed.
- All five changed built-in package `bun run typecheck` commands: PASS.
- Validation does not cover the blocking behavior findings.

## Next step

Run `worktree-fix` with the eight blocking findings above. Use the validation commands above after the fixes.

## Fix workflow

- Status: fixed, not re-reviewed.
- Fixer agent: `issue52-fixer-3`.
- Herdr pane: `wS:p1S`.
- Source branch: `issue-52-bun-runtime`.
- Source worktree: `/run/media/zenobius/Store/Projects/Mine/Github/AgentBoard.worktrees/issue-52-bun-runtime`.
- Handoff: `/tmp/52-fix-handoff.md`.
- Active `ALIGNMENT_ROOT`: `/run/media/zenobius/Store/Projects/Mine/Github/AgentBoard.worktrees/issue-52-bun-runtime`.
- Alignment storage: `repository`.
- Engineering skill: Matt Pocock `diagnosing-bugs`.
- Validation: `moon run agentboard:ts-test agentboard:ts-typecheck agentboard-core:ts-test agentboard-core:ts-typecheck --force && git diff --check origin/main --` plus all five built-in package `bun run typecheck` commands.

## Fix result

- Built-in Actions now read rendered inputs.
- Built-in Sources now paginate and reject duplicate Item identities.
- Built-in subprocesses now use detached process groups on supported platforms.
- Cancellation gates and built-in validation were updated.
- All five built-in package typechecks passed.
- CLI tests: 68 passed.
- Core tests: 10 passed.
- CLI and Core TypeScript checks passed.
- `git diff --check origin/main --` passed.
- The review verdict remains `FAILURE` until a new review replaces it.

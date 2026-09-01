# Review: UserRequest issue 35

## Scope

- UserRequest: `issue 35`
- Ticket: `Cancel Jira collection end-to-end`
- Worktree: `/run/media/zenobius/Store/Projects/Mine/Github/AgentBoard.worktrees/agentboard-issue-35-cancel-jira`
- Source branch: `agentboard/issue-35-cancel-jira`
- Base branch: `main`
- Source commit: `c479035a5097f59eea9ba5f908f0d947661d33a3`
- Base comparison commit: `c479035a5097f59eea9ba5f908f0d947661d33a3` (merge-base of `main` and the source branch)
- Current `main`: `8524c3388d6bf3ed8b5448b6677d8183f879fb15`
- Review diff: `git diff c479035a5097f59eea9ba5f908f0d947661d33a3 -- pkgs/crates/agentboard-source-jira/Cargo.toml pkgs/crates/agentboard-source-jira/src/lib.rs`
- The reviewed changes are uncommitted.

## Standards

No documented-standard violations found.
No Fowler smell worth calling out.

## Findings

1. **Blocking — completion race is not preserved.**
   `pkgs/crates/agentboard-source-jira/src/lib.rs:182` checks the invocation token after the final page and normalization work, immediately before returning `SourceCollection`. Cancellation after collection completion but before that check converts a completed collection into cancellation. This conflicts with the requirement: “Preserve completion when it wins the race.”

2. **Blocking — response body-read cancellation lacks mock-server coverage.**
   The delayed mock response test delays before sending the response. It does not hold an active response body while `response.text()` is reading. The requirement explicitly asks to race cancellation against “body reads.” The completed-response test cancels only after `collect` returns, so it does not exercise a completion-versus-cancellation race.

## Validation

- `cargo test -p agentboard-source-jira`: passed, 16 tests.

## Verdict

FAILURE

## Timestamp

2026-08-08T05:13:36+00:00

## Fix dispatch

- Handoff: `/tmp/35-fix-handoff.md`
- Fixer agent: `jira-fixer`
- Herdr pane: `wM:p8`
- Expected validation: `cargo test -p agentboard-source-jira`

## Follow-up review

- Source branch: `agentboard/issue-35-cancel-jira`
- Base branch: `main`
- Source commit: `c479035a5097f59eea9ba5f908f0d947661d33a3`
- Reviewed diff: `git diff c479035a5097f59eea9ba5f908f0d947661d33a3 -- pkgs/crates/agentboard-source-jira/Cargo.toml pkgs/crates/agentboard-source-jira/src/lib.rs`
- Findings: None. The standards and spec reviews found no blocking issues.
- Validation: `cargo test -p agentboard-source-jira` passed, 16 tests.
- Verdict: `SUCCESS`
- Timestamp: `2026-08-08T05:33:22+00:00`

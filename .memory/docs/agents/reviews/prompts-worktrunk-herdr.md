# Prompt Review: Worktrunk and Herdr Enforcement

Date: 2026-08-24
Scope: `.agentboard.toml`, `.pi/prompts/implement.md`, `.pi/prompts/review.md`, and the `worktree-*` command prompts.

## Verdict

The current prompts do not guarantee Worktrunk or Herdr use.

## Evidence

1. `.agentboard.toml` runs `@agentboard/action-worktree`.
   The action uses plain `git worktree` by ADR 0006.
2. `.agentboard.toml` runs `pi --print --approve` directly.
   This process does not start an agent through Herdr.
3. `.pi/prompts/implement.md` says to use the current AgentBoard-managed worktree.
   It does not require `HERDR_ENV=1`, `wt`, or a Herdr agent identity.
4. `.pi/prompts/review.md` has the same gap.
5. `.pi/prompts/review.md` does not require the review artifact required by `AGENTS.md`.
6. The `worktree-start`, `worktree-fix`, `worktree-review`, and `worktree-submit` prompts already define the desired Worktrunk and Herdr lifecycle.

## Main recommendation

Do not try to enforce this only with prompt text.

Change the runner boundary first:

1. Replace the plain Git worktree action in the AgentBoard pipeline with a runner that uses `wt`.
2. Start the implementation or review agent through Herdr.
3. Pass the worktree path and ticket data in a handoff file.
4. Let the agent prompt fail closed when the Worktrunk or Herdr preflight fails.

The current direct `pi --print` command cannot satisfy the Herdr requirement by itself.

## Proposed prompt preflight

Add this block to both implementation and review prompts:

```md
## Required preflight

- You MUST run inside a Herdr-managed pane. If `HERDR_ENV` is not `1`, stop before changing files or ticket state.
- You MUST use Worktrunk for worktree and branch operations. Use `wt`, not `git worktree`, for these operations.
- You MUST confirm that the current directory is the handoff worktree and that its branch is not the base branch.
- You MUST read the Worktrunk state before work. If the worktree, branch, or Herdr agent identity does not match the handoff, stop and report the mismatch.
- You MUST NOT create a second worktree for the ticket.
- You MUST NOT work in the repository root or a base-branch worktree.
```

Use `git` for diff and commit inspection. Use `wt` for worktree lifecycle operations. Use `herdr` for pane and agent operations.

## Proposed implementation prompt changes

Keep the existing issue and label workflow. Replace the current worktree sentence with:

```md
- Work only in the Worktrunk worktree supplied by the handoff.
- Use the `worktree-start` lifecycle when no valid handoff worktree exists.
- Before changing files, record the worktree path, branch, base branch, and Herdr agent name.
- Run the required validation in the worktree.
- Commit and push only from the source worktree. Do not push the base branch.
- After implementation, leave the worktree open for independent review.
```

Do not tell the implementation agent to run a local review and then submit the PR in the same step. Keep implementation and review independent.

## Proposed review prompt changes

Add these requirements:

```md
- Review only the Worktrunk worktree named in the handoff.
- Do not change source files, commit, merge, push, or remove the worktree.
- Read the relevant ADRs before reviewing.
- Write `.memory/docs/agents/reviews/{ticket-id}.md` before reporting a verdict.
- The artifact MUST contain the scope, source branch, base branch, commit, findings, validation command, validation result, and `SUCCESS` or `FAILURE`.
- Use `FAILURE` for every blocking finding. Include the ADR, file, and line for each finding.
- Use `SUCCESS` only when no blocking findings remain.
```

The existing GitHub label transitions can remain, but they must happen after the artifact is written. If artifact writing fails, do not mark the review complete.

## Prompt consolidation

The repository now has two lifecycle systems:

- `.pi/prompts/implement.md` and `.pi/prompts/review.md` for the AgentBoard GitHub pipeline.
- `worktree-start`, `worktree-fix`, `worktree-review`, `worktree-submit`, and `worktree-finish` for Worktrunk plus Herdr.

Use one lifecycle. The smallest safe change is to make the AgentBoard runner invoke the `worktree-*` lifecycle and reduce `implement.md` and `review.md` to ticket-specific work instructions. Do not maintain two different definitions for worktree ownership, review gates, or completion.

## Required runner changes

The `.agentboard.toml` command needs a wrapper or action with this contract:

1. Require `HERDR_ENV=1`.
2. Use `wt switch --create ... --no-cd` with hooks enabled.
3. Register or open the returned worktree with Herdr.
4. Start the agent in the returned Herdr pane.
5. Pass an absolute handoff file.
6. Wait for the agent lifecycle result.
7. Export the session after the agent exits.
8. Leave the worktree open after implementation or review.

The existing `@agentboard/action-worktree` plus direct `pi --print` command cannot provide this contract.

## Changes not recommended

- Do not add `wt` commands to the prompt while retaining plain `git worktree` in `.agentboard.toml`.
- Do not invoke `herdr` from a process that lacks the current Herdr session context.
- Do not use `--yes` to bypass Worktrunk hook approval automatically. A human must approve project hooks first.
- Do not make the review agent edit code to fix findings. Use `worktree-fix` for that handoff.

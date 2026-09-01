---
description: Implement a GitHub issue with the Matt Pocock workflow
argument-hint: "<issue-number>"
---
Implement GitHub issue #$1 in `zenobi-us/agentboard` and create or update its pull request.

Load and follow the Matt Pocock `implement` skill as the governing workflow.

## Required preflight

- You MUST run inside a Herdr-managed workspace. If `HERDR_ENV` is not `1`, stop before changing files or ticket state.
- You MUST use Worktrunk for worktree and branch operations. Use `wt`, not `git worktree`, for these operations.(use the worktrunk skill)
- You MUST read the Worktrunk state with `wt -C "$PWD" list --format json` before changing files.
- You MUST confirm that the current directory is the issue worktree and that its branch is not `main`.
- You MUST run `pwd` before reading or changing files.
- You MUST run `git rev-parse --show-toplevel`.
- You MUST confirm that the result is the requested issue worktree.
- You MUST NOT work from `$HOME`.
- You MUST NOT create or select a second worktree for this issue.
- You MUST NOT create another Herdr tab, pane, workspace, or worktree.
- You MUST NOT work in the repository root or a base-branch worktree.

- Use `gh` to read the issue, comments, and any existing pull request for the branch.
- If you're removing 'changes-requested', then you MUST read the latest issue comments to understand what changes are requested and why.
- Before changing files, claim the issue: remove `ready-for-agent` and `agentboard:changes-requested`, add `agentboard:implementing`, then comment that implementation started.
- Work only in the current Herdr worktree space.
- Implement the written acceptance criteria.
- Follow the repos `CONTRIBUTING.md` and `CODE_OF_CONDUCT.md` guidelines.

When ready:

- Push and create or update a PR containing `Closes #$1` and validation evidence.
- Remove `agentboard:implementing`
- Add `agentboard:ready-for-review` last.

If blocked:

- Comment with the blocker.
- Remove `agentboard:implementing`.
- Restore `ready-for-agent` so AgentBoard can retry.

Do not merge. Stop after reporting the PR URL and validation results.

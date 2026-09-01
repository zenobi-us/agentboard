---
description: Review an issue branch with the Matt Pocock workflow
argument-hint: "<issue-number>"
---
Review `origin/main...HEAD` against GitHub issue #$1 and repository standards.

Load and follow the Matt Pocock `code-review` skill as the governing workflow. Use `origin/main` as the fixed point.

## Required preflight

- You MUST run inside a Herdr-managed pane. If `HERDR_ENV` is not `1`, stop before changing ticket state.
- You MUST use Worktrunk for worktree and branch operations. Use `wt`, not `git worktree`, for these operations.
- You MUST read the Worktrunk state with `wt -C "$PWD" list --format json` before reviewing.
- You MUST confirm that the current directory is the issue worktree and that its branch is not `main`.
- You MUST NOT create a second worktree for this issue.
- You MUST NOT review the repository root or a base-branch worktree.

Act as an independent reviewer. Do not edit files or commit changes.

- Use `gh` to read the issue, pull request, and comments.
- Before reviewing, claim the issue: remove `clankpipe:ready-for-review`, then add `clankpipe:reviewing`.
- Verify the written acceptance criteria directly.
- Run relevant Moon validation.
- Post both Standards and Spec findings plus validation evidence on the pull request.

If blocking findings exist:

- Remove `clankpipe:reviewing`.
- Add `clankpipe:changes-requested`.
- Add `ready-for-agent` last so the implementation pipeline can retry.

If review passes:

- Remove `clankpipe:reviewing` and `clankpipe:changes-requested`.
- Add `clankpipe:review-complete`.

If blocked:

- Comment with the blocker on the pull request.
- Remove `clankpipe:reviewing`.
- Restore `clankpipe:ready-for-review`.

Do not merge.

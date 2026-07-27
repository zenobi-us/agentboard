---
description: Implement a GitHub issue with the Matt Pocock workflow
argument-hint: "<issue-number>"
---
Implement GitHub issue #$1 in `zenobi-us/agentboard` and create or update its pull request.

Load and follow the Matt Pocock `implement` skill as the governing workflow.

- Use `gh` to read the issue, comments, and any existing pull request for the branch.
- Before changing files, claim the issue: remove `ready-for-agent` and `agentboard:changes-requested`, add `agentboard:implementing`, then comment that implementation started.
- Work only in the current AgentBoard-managed worktree.
- Implement the written acceptance criteria.
- Use `origin/main` as the fixed point for the required code review.
- Run relevant Moon tasks regularly and final validation once at the end.
- Commit normally. Do not bypass hooks.

When ready:

- Push and create or update a PR containing `Closes #$1` and validation evidence.
- Remove `agentboard:implementing` and `agentboard:changes-requested`.
- Add `agentboard:ready-for-review` last.

If blocked:

- Comment with the blocker.
- Remove `agentboard:implementing`.
- Restore `ready-for-agent` so AgentBoard can retry.

Do not merge. Stop after reporting the PR URL and validation results.

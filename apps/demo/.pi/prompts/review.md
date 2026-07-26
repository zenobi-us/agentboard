Review the GitHub issue and associated pull request.

Act as an independent reviewer. Do not edit files or commit changes.

- Use `gh` to read the issue and PR and inspect the diff.
- Before reviewing, claim the issue: remove `agentboard:ready-for-review`, then add `agentboard:review-in-progress`.
- Verify the written acceptance criteria directly.
- Run `bun run lint` and include the result in the review evidence.

If changes are required:

- Comment precise findings on the issue.
- Remove `agentboard:review-in-progress` and `agentboard:ready-for-review`.
- Add `agentboard:changes-requested` and `agentboard:ready-for-agent`.

If the change passes:

- Comment with review and lint evidence.
- Remove `agentboard:review-in-progress`, `agentboard:ready-for-review`, and `agentboard:changes-requested` when present.
- Add `agentboard:review-complete`.

If blocked or unable to finish:

- Comment with the blocker.
- Remove `agentboard:review-in-progress`.
- Restore `agentboard:ready-for-review` so AgentBoard can retry.

Do not merge the PR.
Stop after reporting the transition you applied.

Issue: $1
UserRequest: $ARGUMENTS

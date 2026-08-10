Review the GitHub issue and associated pull request.

Act as an independent reviewer. Do not edit files or commit changes.

- Use `gh` to read the issue and PR and inspect the diff.
- AgentBoard claims the issue before it launches this prompt.
- Verify the written acceptance criteria directly.
- Run and include the result in the review evidence.

If changes are required:

- Comment on the PR with precise findings and requested changes.
- Remove `agentboard:review-in-progress` and `agentboard:ready-for-review`.
- Add `agentboard:changes-requested` and `agentboard:ready-for-agent`.

If the change passes:

- Comment on the PR with review and lint evidence.
- use the gh cli to approve and merge the PR.
- Remove `agentboard:review-in-progress`, `agentboard:ready-for-review`, and `agentboard:changes-requested` when present.
- Add `agentboard:review-complete`.

If blocked or unable to finish:

- Comment with the blocker on the PR.
- Remove `agentboard:review-in-progress`.
- Restore `agentboard:ready-for-review` so AgentBoard can retry.


Issue: $1
UserRequest: $ARGUMENTS

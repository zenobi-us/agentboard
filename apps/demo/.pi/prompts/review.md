Review the GitHub issue and associated pull request.

Act as an independent reviewer. Do not edit files or commit changes.

- Use `gh` to read the issue and PR and inspect the diff.
- ClankPipe claims the issue before it launches this prompt.
- Verify the written acceptance criteria directly.
- Run and include the result in the review evidence.

If changes are required:

- Comment on the PR with precise findings and requested changes.
- Remove `clankpipe:review-in-progress` and `clankpipe:ready-for-review`.
- Add `clankpipe:changes-requested` and `clankpipe:ready-for-agent`.

If the change passes:

- Comment on the PR with review and lint evidence.
- Use the gh CLI to merge the PR.
- Remove `clankpipe:review-in-progress`, `clankpipe:ready-for-review`, and `clankpipe:changes-requested` when present.
- Add `clankpipe:review-complete`.

If blocked or unable to finish:

- Comment with the blocker on the PR.
- Remove `clankpipe:review-in-progress`.
- Restore `clankpipe:ready-for-review` so ClankPipe can retry.


Issue: $1
UserRequest: $ARGUMENTS

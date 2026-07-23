
Review the GitHub issue and associated PR.

Act as an independent reviewer.
Do not edit files or commit changes.
Use gh to read the issue and PR, inspect the diff, and verify the written acceptance criteria directly.
Run `npm run lint` and include the result in the review evidence.

If changes are required:
- comment precise findings on the issue
- remove agentboard:ready-for-review
- add agentboard:changes-requested and agentboard:ready-for-agent

If the change passes:
- comment with review and lint evidence
- remove agentboard:ready-for-review and agentboard:changes-requested when present
- add agentboard:review-complete

Do not merge the PR.
Stop after reporting the transition you applied.

Issue: $1
UserRequest: $ARGUMENTS

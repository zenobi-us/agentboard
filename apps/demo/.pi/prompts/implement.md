
Implement GitHub issue and create a PR.

- Use gh to read the issue and any existing PR for the branch.
- Work only in this worktree.
- Run the exact task test from the issue, then run ./test.sh.
- Commit the change, push, and create or update a PR whose body contains 'Closes #$issue'.

When the branch is ready: 

- remove labels agentboard:ready-for-agent and agentboard:changes-requested when present
- then add agentboard:ready-for-review. 

Do not merge the PR. 
Stop after reporting the PR URL and test results.

Issue: $1


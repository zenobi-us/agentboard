
Implement the GitHub issue and create a PR.

- Use gh to read the issue and any existing PR for the branch.
- Work only in this worktree.
- Implement the issue's written acceptance criteria directly.
- Commit normally. Husky runs lint-staged and ESLint against staged HTML and CSS files.
- Fix hook failures; do not bypass hooks with `--no-verify`.
- Push and create or update a PR whose body contains `Closes #$issue`.

When the branch is ready:

- remove labels agentboard:ready-for-agent and agentboard:changes-requested when present
- then add agentboard:ready-for-review

Do not merge the PR.
Stop after reporting the PR URL and validation results.

Issue: $1

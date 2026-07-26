Implement the GitHub issue and create or update its pull request.

- Use `gh` to read the issue and any existing PR for the branch.
- Before doing other work, claim the issue: remove `agentboard:ready-for-agent` and `agentboard:changes-requested`, then add `agentboard:in-progress`.
- Comment on the issue to acknowledge that work has started.
- Work only in this worktree.
- Implement the issue's written acceptance criteria directly.
- Commit normally. Husky runs lint-staged and ESLint against staged HTML and CSS files.
- Fix hook failures; do not bypass hooks with `--no-verify`.

When the branch is ready:

- Push and create or update a PR whose body describes the "why" of the changes. Ensure the PR contains `Closes #$1`, validation results, and review handoff.
- Remove `agentboard:in-progress`, `agentboard:ready-for-agent`, and `agentboard:changes-requested` when present.
- Add `agentboard:ready-for-review` last, so the watcher sees the completed handoff state.

If blocked or unable to finish:

- Comment with the blocker.
- Remove `agentboard:in-progress`.
- Restore `agentboard:ready-for-agent` so AgentBoard can retry.

Do not merge the PR.
Stop after reporting the PR URL and validation results.

Issue: $1

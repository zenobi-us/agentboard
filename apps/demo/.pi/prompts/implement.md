Implement the GitHub issue and create or update its pull request.

- Use `gh` to read the issue and any existing PR for the branch.
- ClankPipe claims the issue before it launches this prompt.
- Comment on the issue to acknowledge that work has started.
- Work only in this worktree.
- Implement the issue's written acceptance criteria directly.
- Commit normally. Husky runs lint-staged and ESLint against staged HTML and CSS files.
- Fix hook failures; do not bypass hooks with `--no-verify`.

When the branch is ready:

- Push and create or update a PR whose body describes the "why" of the changes.
- Use conventional commit messages for the PR title. include the issue number in the title.
- Ensure the pr description contains validation results, and review handoff.
- Remove `clankpipe:in-progress`, `clankpipe:ready-for-agent`, and `clankpipe:changes-requested` when present.
- Add `clankpipe:ready-for-review` last, so the watcher sees the completed handoff state.

If blocked or unable to finish:

- Comment with the blocker.
- Remove `clankpipe:in-progress`.
- Restore `clankpipe:ready-for-agent` so ClankPipe can retry.

Do not merge the PR.
Stop after reporting the PR URL and validation results.

Issue: $1

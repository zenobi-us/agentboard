---
backend: github
---

# Issue tracker: GitHub

Issues and PRDs for this repo live in GitHub Issues for `zenobi-us/clankpipe`. Use the `gh` CLI for all issue tracker operations.

## Conventions

- **Create an issue**: `gh issue create --title "..." --body "..."`
- **Read an issue**: `gh issue view <number> --comments`
- **List issues**: `gh issue list --state open --json number,title,body,labels,comments`
- **Comment on an issue**: `gh issue comment <number> --body "..."`
- **Apply / remove labels**: `gh issue edit <number> --add-label "..."` / `--remove-label "..."`
- **Close**: `gh issue close <number> --comment "..."`

## Issue dependencies

Use GitHub issue dependencies to record work that blocks other work. Do not record dependency data only in issue body text.

- Create blockers before the issues that depend on them.
- Use `--blocked-by` when creating an issue that depends on another issue.
- Use `--blocking` when creating an issue that blocks another issue.
- Each flag accepts a comma-separated list of issue numbers or URLs.
- Add relationships later with `gh issue edit <number> --add-blocked-by <number>` or `--add-blocking <number>`.
- Remove relationships with `--remove-blocked-by` or `--remove-blocking`.
- Verify relationships with `gh issue view <number> --json blockedBy,blocking`.
- Use the native relationship as the source of truth. Keep issue bodies focused on scope and acceptance criteria.
- Do not create circular dependencies.
- Use the `ready-for-agent` label only when all required information is present. A blocked issue can still use this label when its acceptance criteria are complete.

Example:

```bash
gh issue create --title "Add the wrapper" --body-file wrapper.md --blocked-by 75
gh issue create --title "Add recovery" --body-file recovery.md --blocked-by 74,75,76
```

Infer the repo from `git remote -v`; `gh` does this automatically inside this clone.

## When a skill says "publish to the issue tracker"

Create a GitHub issue in `zenobi-us/clankpipe`.

## When a skill says "fetch the relevant ticket"

Run `gh issue view <number> --comments` and include labels in the result when triage state matters.

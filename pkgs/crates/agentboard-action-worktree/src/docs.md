---
title: Worktree action
---

# Worktree action

Use `agentboard/worktree` to ensure each item has a Git worktree on its
requested branch. The Action creates an absent worktree, reuses a matching
worktree, or safely switches a clean managed worktree.

```toml
[[sources.actions]]
uses = "agentboard/worktree"

[sources.actions.with]
repo = "/home/me/dev/myrepo"
root = "/home/me/dev/myrepo.trees/{{ item.id }}"
branch = "feat/{{ item.id }}-{{ item.title | slugify }}"
```

`repo` is the source repository. `root` is a separate worktree path. `branch`
is a local branch name to create or switch to for the item. Existing roots must
be exact worktree roots from `repo`; switching refuses tracked or untracked
changes and branches already checked out elsewhere.

Pair it with `agentboard/run-cmd` when you want to open the new worktree in a
terminal or agent session.

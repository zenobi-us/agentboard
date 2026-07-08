---
title: Create worktree action
---

# Create worktree action

Use `agentboard/create-worktree` to create or reuse a Git worktree for each
item.

```toml
[[sources.actions]]
uses = "agentboard/create-worktree"

[sources.actions.with]
repo = "/home/me/dev/myrepo"
root = "/home/me/dev/myrepo.trees/{{ item.id }}"
branch = "feat/{{ item.id }}-{{ item.title | slugify }}"
```

`repo` is the source repository. `root` is the worktree path. `branch` is the
branch name to create or check out for the item.

Pair it with `agentboard/run-cmd` when you want to open the new worktree in a
terminal or agent session.

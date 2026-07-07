# Use plain git worktree for the built-in worktree action

`agentboard/create-worktree` uses plain `git worktree` instead of Worktrunk so workspace configs stay portable outside one developer's local toolchain. Worktrunk integration can still be added later as a separate action if its hooks and conventions become part of a workspace's intended behavior.

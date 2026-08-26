---
title: LLM action
---

Use `@agentboard/action-llm` to run a Pi prompt directly or in a terminal.

```toml
[[sources.actions]]
uses = "@agentboard/action-llm"

[sources.actions.with]
prompt = "Implement {{ item.reference_id }}: {{ item.title }}"
provider = "openai"
model = "gpt-5"
thinking = "high"
mode = "foreground"
cwd = "/home/me/dev/project"
```

Set exactly one of `prompt` or `prompt_file`. The prompt file path is read after
AgentBoard renders action inputs.

Use `worktree` to create or reuse a clean Git worktree before the agent starts.
The action does not switch an existing worktree to another branch.

```toml
[sources.actions.with.worktree]
repo = "/home/me/dev/project"
root = "/home/me/dev/project.trees/{{ item.id }}"
branch = "agentboard/{{ item.id }}"
```

Set `terminal` to `zellij`, `herdr`, `tmux`, or `generic`. Terminal launches
return after the terminal command starts. Direct launches wait for Pi in
`foreground` mode and return after launch in `background` mode.

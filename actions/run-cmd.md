# Run command action (/actions/run-cmd)



# Run Command Action [#run-command-action]

Use `agentboard/run-cmd` to run a shell command for each collected item.

```toml
[[sources.actions]]
uses = "agentboard/run-cmd"

[sources.actions.with]
cmd = "echo {{ item.id }} {{ item.title }}"
```

AgentBoard runs the command through `sh -c`. Templates can use the collected
item and source fields.

The command receives these environment variables:

* `AGENTBOARD_WORKSPACE_ID`
* `AGENTBOARD_SOURCE_ID`
* `AGENTBOARD_ITEM_ID`

Set `cwd` when the command should run from a specific directory.

```toml
[sources.actions.with]
cwd = "/home/me/dev/myrepo"
cmd = "pi --prompt '{{ item.title }}'"
```

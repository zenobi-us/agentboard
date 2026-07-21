# Pi, QMD and Zellij

This example turns local Markdown task files into isolated Pi sessions managed
inside one Zellij dashboard.

```text
tasks/*.md -> QMD -> AgentBoard watch -> Git worktree -> Zellij tab -> Pi
                                      \-> agent sidebar + top tabbar
```

- **Agent:** [Pi](https://github.com/badlogic/pi-mono)
- **Task source:** [QMD](https://github.com/tobi/qmd)
- **Multiplexer:** [Zellij](https://zellij.dev/)
- **Agent sidebar:** [zellij-agent-threads](https://github.com/zenobi-us/zellij-agent-threads)
- **Top tabbar:** [zellij-tabbar](https://github.com/zenobi-us/zellij-plugins/tree/main/pkgs/zellij-tabbar)

Zellij keeps the dashboard and Pi processes alive after detaching from a local or
remote terminal. Reattach later and continue from the same tabs.

## Included files

```text
.agentboard.toml       AgentBoard QMD source and Actions
.qmd/index.yml         project-local QMD collection config
tasks/                 ready, blocked, and completed Markdown tasks
demo-repo/             toy Git repository initialized by setup.sh
setup.sh               checks tools, initializes Git, and indexes QMD
launch-pi-task.sh      opens a Zellij tab and starts Pi
zellij-layout.kdl      top tabbar, left agent sidebar, and queue pane
```

The example creates one Git worktree per launched task. Pi never edits the
baseline `demo-repo` checkout directly.

## Prerequisites

Install:

- AgentBoard from this repository: `moon run agentboard:install`
- QMD: `bun install -g @tobilu/qmd`
- Zellij 0.45.x
- Pi, with a model provider configured
- Git, Bun, proto, Moon, and Rust for building the two Zellij plugins

Install `zellij-agent-threads` and its Pi extension:

```bash
git clone https://github.com/zenobi-us/zellij-agent-threads.git
cd zellij-agent-threads
proto install
bun install
moon run zellij-plugin:build
moon run pi-extension:install
install -Dm644 \
  pkgs/plugins/zellij-plugin/target/wasm32-wasip1/release/zellij-plugin-agent-threads.wasm \
  ~/.config/zellij/plugins/zellij-agent-threads.wasm
```

Install `zellij-tabbar`:

```bash
git clone https://github.com/zenobi-us/zellij-plugins.git
cd zellij-plugins
proto install
moon run zellij-tabbar:install
```

Both plugin paths are referenced directly by `zellij-layout.kdl`.

## 1. Prepare the local task repository

Run every command below from this example directory:

```bash
cd apps/cli/docs/examples/pi-qmd-and-zellij
./setup.sh
```

`setup.sh`:

1. verifies required commands and plugin WASM files;
2. initializes `demo-repo` as an independent Git repository;
3. loads `.qmd/index.yml`;
4. runs `qmd update` and `qmd embed`.

The first embedding run downloads QMD's local models and can take time.

Inspect the task collection:

```bash
qmd collection list
qmd query $'intent: Find the task explicitly queued for AgentBoard execution.\nlex: agentboard-ready' \
  -c agentboard-tasks
```

Three lifecycle examples are included:

- `TASK-001` — `ready`, with the `agentboard-ready` queue marker.
- `TASK-002` — `blocked`, waiting for a flag-name decision.
- `TASK-003` — `done`, retained as task history.

The AgentBoard source uses the exact `agentboard-ready` marker and `limit = 1`,
so only `TASK-001` launches initially.

## 2. Check the AgentBoard plan

Run a dry run before opening the dashboard:

```bash
agentboard run .agentboard.toml --dry-run
```

The configured Actions are:

1. `agentboard/create-worktree` creates `.worktrees/task-001` on branch
   `task/task-001`.
2. `agentboard/run-cmd` calls `launch-pi-task.sh` from that worktree.
3. The launcher creates a Zellij tab and types an interactive Pi command whose
   prompt includes `tasks/TASK-001.md`.

The live Action requires a running Zellij session. Dry-run does not create the
worktree or tab.

## 3. Start the dashboard

```bash
zellij --session agentboard-demo \
  --new-session-with-layout "$PWD/zellij-layout.kdl"
```

The `queue` tab starts:

```bash
agentboard watch .agentboard.toml --interval 30s
```

Each tab uses the same frame:

```text
+-------------------------------------------------------+
| zellij-tabbar: session | queue | TASK-001 | +          |
+-------------------------------+-----------------------+
| zellij-agent-threads          | focused tab content   |
| - Pi state                    | AgentBoard watch or    |
| - task/title                  | interactive Pi         |
| - pane/worktree/model         |                       |
+-------------------------------+-----------------------+
```

On first launch, each plugin can ask for Zellij permissions. Click the top
one-row tabbar or left sidebar prompt, then press `y`. The tabbar needs
`ReadApplicationState`, `ChangeApplicationState`, and
`OpenTerminalsOrPlugins`. The sidebar needs application-state access to list and
focus Pi panes.

When AgentBoard sees `TASK-001`, it creates the worktree and opens a `TASK-001`
tab. The Pi extension installed with `zellij-agent-threads` reports that session
to the left sidebar.

AgentBoard records successful **launches**, not Pi task completion. Update task
frontmatter when the work is reviewed or merged.

## 4. Detach and reconnect

Detach using your normal Zellij keybinding. The session and Pi process continue
running.

```bash
zellij attach agentboard-demo
```

This works the same way over SSH: detach before disconnecting, then reattach on
the remote host.

## 5. Advance the task lifecycle

After `TASK-001` is complete, edit its frontmatter:

```yaml
status: "done"
queue: "agentboard-archive"
```

Resolve the blocker in `TASK-002`, then change its frontmatter to:

```yaml
status: "ready"
queue: "agentboard-ready"
```

Refresh QMD:

```bash
qmd update
qmd embed
```

Within 30 seconds, AgentBoard finds `TASK-002`, creates its worktree, and opens a
new Pi tab. Existing successful launch records prevent the same task Action from
being launched repeatedly.

## Cleanup

Stop the watch pane or kill the Zellij session:

```bash
zellij kill-session agentboard-demo
```

Remove generated worktrees and the local QMD database:

```bash
git -C demo-repo worktree list
if [ -d .worktrees ]; then
  for worktree in "$PWD"/.worktrees/*; do
    [ -d "$worktree" ] || continue
    git -C demo-repo worktree remove --force "$worktree"
  done
fi
git -C demo-repo worktree prune
rm -f .qmd/index.sqlite
```

The nested `demo-repo/.git` directory is generated by `setup.sh`. Remove it to
reset the toy repository completely:

```bash
rm -rf demo-repo/.git
```

---
title: Quickstart
---

# Quickstart

This demo creates a private throwaway repository from the [AgentBoard demo template](https://github.com/zenobi-us/agentboard-demo-template), seeds twelve coding issues, and runs three GitHub Sources through one AgentBoard Watch:

```text
ready-for-agent -> implementation -> ready-for-review -> review
       ^                                      |             |
       |                                      | pass        | changes requested
       +--------------------------------------+-------------+
                                              |
                                  manual merge + cleanup approval
                                              |
                                  remove worktree + Zellij tab
```

AgentBoard records successful launches. GitHub labels, PR state, and your explicit actions record lifecycle completion.

## Prerequisites

The demo supports Linux, macOS, and WSL. Install and configure:

- AgentBoard
- [GitHub CLI](https://cli.github.com/) authenticated with `gh auth login`
- Git
- [Zellij](https://zellij.dev/)
- Neovim
- [Pi](https://github.com/badlogic/pi-mono) with a model provider configured

## 1. Create the private demo repository

### GitHub CLI

Replace `OWNER` with your GitHub user or organization:

```sh
gh repo create OWNER/agentboard-quickstart-demo \
  --private \
  --template zenobi-us/agentboard-demo-template
```

### GitHub web UI

1. Open [`zenobi-us/agentboard-demo-template`](https://github.com/zenobi-us/agentboard-demo-template).
2. Select **Use this template**, then **Create a new repository**.
3. Choose an owner and name, select **Private**, then create the repository.

The generated repository automatically runs **Initialize AgentBoard demo**. Wait for that workflow to pass. It creates the lifecycle labels and twelve issues, then renders the repository-specific `.agentboard.toml`.

Clone the initialized repository:

```sh
gh repo clone OWNER/agentboard-quickstart-demo
cd agentboard-quickstart-demo
```

If you cloned before initialization completed, run `git pull --ff-only` after the workflow passes.

The generated Workspace contains three configured GitHub Sources:

1. **implement** — open issues labelled `agentboard:ready-for-agent`;
2. **review** — open issues labelled `agentboard:ready-for-review`;
3. **cleanup** — closed issues labelled both `agentboard:review-complete` and `agentboard:cleanup-approved`.

## 2. Start Watch and release two issues

Start or attach to the demo Zellij session:

```sh
XDG_DATA_HOME="$PWD/.data" \
  zellij attach --create agentboard-demo options \
  --default-cwd "$PWD" \
  --default-layout "$PWD/zellij-layout.kdl"
```

The `queue` tab automatically runs:

```sh
agentboard watch .agentboard.toml --interval 15s
```

Open the temporary repository from another terminal:

```sh
gh repo view --web
```

Choose two issues and apply `agentboard:ready-for-agent`.

GitHub Search can take several seconds to index a label change. On the next matching run, AgentBoard creates or reuses the issue worktree and opens its Zellij tab. Each tab keeps Neovim visible while Pi runs in a separate pane.

## 3. Observe implementation and review

The implementation Pi session reads the issue, runs its task-specific shell test, commits, pushes, and creates or updates a PR containing `Closes #<issue>`. It then moves the issue to `agentboard:ready-for-review`.

The review Source starts a separate reviewer Pi session in the same issue tab:

- **Pass:** Pi comments with review evidence and applies `agentboard:review-complete`. Review and merge the PR yourself in GitHub.
- **Changes requested:** Pi comments, applies `agentboard:changes-requested`, and returns the issue to `agentboard:ready-for-agent`. The implementation Source resumes the deterministic implementation session in the existing worktree.

After merging both accepted PRs, apply `agentboard:cleanup-approved` to their closed issues. The cleanup Source removes each worktree and closes its issue tab.

## 4. Tear down the throwaway demo

From the generated repository root, run:

```sh
./teardown.sh
```

`teardown.sh` verifies the repository's immutable ID, template origin, private visibility, and repository ownership marker before deleting the Zellij session, worktrees, remote repository, and local clone. If GitHub refuses repository deletion, run `gh auth refresh -h github.com -s delete_repo` and repeat teardown.

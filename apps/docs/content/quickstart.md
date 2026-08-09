---
title: Quickstart
---

# Quickstart

This demo copies the repository's `apps/demo` fixture into a private throwaway GitHub repository, creates twelve coding issues, and runs three GitHub Sources through one AgentBoard Watch:

```text
ready-for-agent
    -> implementation terminal
    -> ready-for-review
    -> review terminal
          |
          +--> changes requested --> ready-for-agent
          |
          +--> review complete + merged PR
                    -> cleanup approval
                    -> remove worktree
```

AgentBoard records successful actions. GitHub labels, PR state, and your cleanup approval record lifecycle completion.

## Prerequisites

The demo uses a launcher to open Pi in a new pane, tab, or terminal window. Install and configure:

- AgentBoard
- [GitHub CLI](https://cli.github.com/) authenticated with `gh auth login`
- Git
- [Bun](https://bun.sh/)
- `curl`, `tar`, and `jq`
- [Pi](https://github.com/badlogic/pi-mono) with a model provider configured
- One launcher: [Zellij](https://zellij.dev/), Herdr, GNOME Terminal, xterm, Konsole, Kitty, Alacritty, or WezTerm

## 1. Create the private demo repository

From the directory where you want the local clone, run:

```sh
curl https://raw.githubusercontent.com/zenobi-us/agentboard/refs/heads/main/apps/demo/setup.sh | sh
```

When prompted, enter the new GitHub repository as `OWNER/agentboard-quickstart-demo`. To skip the prompt, pass `REPO` to the shell running the downloaded script:

```sh
curl https://raw.githubusercontent.com/zenobi-us/agentboard/refs/heads/main/apps/demo/setup.sh \
  | REPO=OWNER/agentboard-quickstart-demo sh
```

The setup script:

1. creates a private GitHub repository;
2. clones it into `./agentboard-quickstart-demo`;
3. downloads and copies only AgentBoard's `apps/demo` directory;
4. renders the repository-specific query in `.agentboard.toml`;
5. installs ESLint, Husky, and lint-staged;
6. creates a pre-commit hook that lints staged `*.html` and `*.css` files with ESLint;
7. commits and pushes the demo;
8. creates the lifecycle labels and twelve GitHub issues from `.issues/*.json`.

Enter the generated repository:

```sh
cd agentboard-quickstart-demo
```

The generated Workspace contains three configured GitHub Sources:

1. **implement** — open issues labelled `agentboard:ready-for-agent`;
2. **review** — open issues labelled `agentboard:ready-for-review`;
3. **cleanup** — closed issues labelled both `agentboard:review-complete` and `agentboard:cleanup-approved`.

## 2. Start Watch Mode and release two issues

Start AgentBoard from the standalone repository:

```sh
agentboard run .agentboard.toml --watch --interval 15s
```

The launcher uses Herdr when the process runs inside Herdr, Zellij when it runs inside Zellij, and then the first supported desktop terminal. Set `AGENTBOARD_LAUNCHER` to select one explicitly:

```sh
AGENTBOARD_LAUNCHER=gnome-terminal agentboard run .agentboard.toml --watch --interval 15s
AGENTBOARD_LAUNCHER=xterm agentboard run .agentboard.toml --watch --interval 15s
AGENTBOARD_LAUNCHER=zellij AGENTBOARD_LAUNCH_MODE=tab agentboard run .agentboard.toml --watch --interval 15s
```

From another terminal, list the seeded issues and release two of them:

```sh
gh issue list
gh issue edit <number> --add-label agentboard:ready-for-agent
gh issue edit <number> --add-label agentboard:ready-for-agent
```

GitHub Search can take several seconds to index a label change. On the next matching run, AgentBoard creates or reuses the issue worktree, installs its JavaScript dependencies with Bun, and launches Pi through the selected launcher.

## 3. Observe implementation and review

The implementation Pi session reads the issue and implements its written acceptance criteria. On commit, Husky runs lint-staged and ESLint against staged HTML and CSS files. Pi fixes hook failures, pushes, and creates or updates a PR containing `Closes #<issue>`. It then moves the issue to `agentboard:ready-for-review`.

The review Source launches a separate reviewer Pi session in the same issue worktree:

- **Pass:** Pi comments with review evidence, approves and merges the PR, and applies `agentboard:review-complete`. Confirm that the issue is closed.
- **Changes requested:** Pi comments, applies `agentboard:changes-requested`, and returns the issue to `agentboard:ready-for-agent`. The implementation Source launches another Pi session in the existing worktree.

After an accepted PR is merged, apply `agentboard:cleanup-approved` to its closed issue. The cleanup Source removes the issue worktree.

## 4. Tear down the throwaway demo

Stop AgentBoard with <kbd>Ctrl</kbd>+<kbd>C</kbd>. Record the repository name, then delete the private GitHub repository:

```sh
repo="$(gh repo view --json nameWithOwner --jq .nameWithOwner)"
gh repo delete "$repo" --yes
```

GitHub may require the `delete_repo` scope. Grant it and retry if deletion fails:

```sh
gh auth refresh -h github.com -s delete_repo
```

After remote deletion succeeds, leave the directory and remove the local throwaway clone.

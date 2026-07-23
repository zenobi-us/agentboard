---
title: Quickstart
---

# Quickstart

This demo copies the repository's `apps/demo` fixture into a private throwaway GitHub repository, creates twelve coding issues, and runs three GitHub Sources through one AgentBoard Watch:

```text
ready-for-agent -> implementation terminal -> ready-for-review -> review terminal
       ^                                                    |
       |                                    changes requested|
       +----------------------------------------------------+
                                                            |
                                              review complete + manual merge
                                                            |
                                                   cleanup approval
                                                            |
                                                   remove worktree
```

AgentBoard records successful actions. GitHub labels, PR state, and your explicit merge and cleanup approval record lifecycle completion.

## Prerequisites

The demo targets a Linux desktop with a default terminal configured through `xdg-terminal-exec`. Install and configure:

- AgentBoard
- [GitHub CLI](https://cli.github.com/) authenticated with `gh auth login`
- Git
- [Bun](https://bun.sh/)
- `xdg-terminal-exec`
- [Pi](https://github.com/badlogic/pi-mono) with a model provider configured

## 1. Create the private demo repository

Clone AgentBoard and copy the demo into a standalone directory:

```sh
gh repo clone zenobi-us/agentboard
cp -a agentboard/apps/demo agentboard-quickstart-demo
cd agentboard-quickstart-demo
```

Initialize the standalone repository, then create and push a private GitHub repository. Replace `OWNER` with your GitHub user or organization:

```sh
git init
git add .
git commit -m "chore: initialize AgentBoard demo"
gh repo create OWNER/agentboard-quickstart-demo \
  --private \
  --source=. \
  --remote=origin \
  --push
```

Run the setup script:

```sh
./setup.sh
```

`setup.sh` verifies the required commands and GitHub authentication, renders the repository-specific query in `.agentboard.toml`, creates the lifecycle labels, and creates GitHub issues directly from `.issues/*.json`. Re-running it skips issues whose titles already exist.

The generated Workspace contains three configured GitHub Sources:

1. **implement** — open issues labelled `agentboard:ready-for-agent`;
2. **review** — open issues labelled `agentboard:ready-for-review`;
3. **cleanup** — closed issues labelled both `agentboard:review-complete` and `agentboard:cleanup-approved`.

## 2. Start Watch and release two issues

Start AgentBoard from the standalone repository:

```sh
agentboard watch .agentboard.toml --interval 15s
```

From another terminal, list the seeded issues and release two of them:

```sh
gh issue list
gh issue edit <number> --add-label agentboard:ready-for-agent
gh issue edit <number> --add-label agentboard:ready-for-agent
```

GitHub Search can take several seconds to index a label change. On the next matching run, AgentBoard creates or reuses the issue worktree and launches Pi in a new terminal through `xdg-terminal-exec`.

## 3. Observe implementation and review

The implementation Pi session reads the issue, runs its task-specific test and the full HTML/CSS validation, commits, pushes, and creates or updates a PR containing `Closes #<issue>`. It then moves the issue to `agentboard:ready-for-review`.

The review Source launches a separate reviewer Pi session in the same issue worktree:

- **Pass:** Pi comments with review evidence and applies `agentboard:review-complete`. Review and merge the PR yourself in GitHub.
- **Changes requested:** Pi comments, applies `agentboard:changes-requested`, and returns the issue to `agentboard:ready-for-agent`. The implementation Source launches another Pi session in the existing worktree.

After merging an accepted PR, apply `agentboard:cleanup-approved` to its closed issue. The cleanup Source removes the issue worktree.

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

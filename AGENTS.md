# AGENTS.md

## Project

`agentboard` is a Moon + proto monorepo for a Rust CLI that collects task-tracking items from external and local sources, stores normalized local copies, then runs workspace-configured actions.

- `apps/cli`: Rust CLI.
- `pkgs/crates/agentboard-core`: shared model/types.
- `pkgs/crates/agentboard-source-*`: one crate per source adapter.
- `pkgs/crates/agentboard-action-*`: one crate per action executor.
- `apps/docs`: docs app scaffold.
- `pkgs/tools/deployment`: release/deployment helper scripts.

Also read nested `AGENTS.md` files in apps/pkgs.

## Runtime and tooling

- Use Bun for JS package management.
- Use Moon for repo tasks.
- Use proto for tool versions.
- Use Rust for the CLI.
- Do not use root mise tasks; use package `moon.yml` tasks.

## Common commands

```bash
bun install
moon query projects
moon run agentboard:build
moon run agentboard:test
moon run docs:dev
```

## Coding workflow

- Keep AgentBoard workspace config boring and explicit.
- Keep source adapters isolated from action execution.
- Store raw source payloads so normalized schema can stay small.
- Prefer item-scoped failures over whole-run failure unless config says fail fast.

## Agent skills

### Issue tracker

Issues are tracked in GitHub Issues for `zenobi-us/agentboard`. See `.memory/docs/agents/issue-tracker.md`.

### Triage labels

Triage labels use the default canonical vocabulary. See `.memory/docs/agents/triage-labels.md`.

### Domain docs

Domain docs use a multi-context layout. See `.memory/docs/agents/domain.md`.

### ADRs and reviews

Architecture decisions live under `.memory/docs/adr/`.

Review artifacts live under `.memory/docs/agents/reviews/`.

Agents MUST read relevant ADRs before changing code.

Agents MUST write worktree review artifacts to `.memory/docs/agents/reviews/{ticket-id}.md`.


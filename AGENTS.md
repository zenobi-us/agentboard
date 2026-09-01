# AGENTS.md

## Project

`agentboard` is a Moon + proto monorepo for a Bun CLI that collects task-tracking items from external and local sources, stores normalized local copies, then runs workspace-configured actions.

- `apps/cli`: Bun CLI.
- `pkgs/crates/clankpipe-core`: shared model/types.
- `pkgs/crates/clankpipe-source-*`: one package per source adapter.
- `pkgs/crates/clankpipe-action-*`: one package per action executor.
- `apps/docs`: docs app scaffold.
- `pkgs/tools/deployment`: release/deployment helper scripts.

Also read nested `AGENTS.md` files in apps/pkgs.

## Runtime and tooling

- Use Bun for JS package management.
- Use Moon for repo tasks.
- Use proto for tool versions.
- Use Bun and TypeScript for the CLI.
- Do not use root mise tasks; use package `moon.yml` tasks.

## Common commands

```bash
bun install
moon query projects
moon run clankpipe:build
moon run clankpipe:test
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

### ADRs and agent artifacts

Architecture decisions live under `.memory/docs/adr/`.

Review and workflow artifacts live under `.memory/docs/agents/`.

For this repository, `.memory/docs/agents/` is the canonical location for these artifacts. Generic skill references to `docs/agents/` or `.scratch/` do not apply.

Agents MUST read relevant ADRs before changing code.

Agents MUST write the current worktree review to `.memory/docs/agents/reviews/{ticket-id}.md`.

Agents MUST write the workflow record to `.memory/docs/agents/workflows/{ticket-id}.md` when a workflow record is required.

Ticket artifacts MUST use the ticket ID as the filename. Non-ticket artifacts MUST use the `manual-<slug>.md` filename form.

Agents MUST overwrite the current review for the same ticket. Git history preserves prior review versions.


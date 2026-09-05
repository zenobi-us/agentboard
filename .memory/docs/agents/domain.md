# Domain Docs

How engineering skills should consume this repo's domain documentation when exploring the codebase.

## Layout

This repo uses a **multi-context** domain-doc layout.

Read these before exploring code:

- `CONTEXT-MAP.md` at the repo root. It defines each monorepo context and its scoped ADR directory.
- The `CONTEXT.md` file listed for the app/package being changed.
- The scoped ADR directory listed for that app/package, if it exists.
- `.memory/docs/adr/` for system-wide ADRs, if files exist there.
- If a change crosses contexts, read every touched context and ADR scope.

Current context docs:

- `apps/cli/CONTEXT.md` for CLI commands, config loading, runtime orchestration, local store, template rendering, and dispatch.
- `pkgs/packages/clankpipe-core/CONTEXT.md` for shared model/config/result types.
- `pkgs/packages/clankpipe-source-qmd/CONTEXT.md` for the QMD source adapter.
- `pkgs/packages/clankpipe-source-jira/CONTEXT.md` for the Jira source adapter.
- `pkgs/packages/clankpipe-action-run-cmd/CONTEXT.md` for the shell command action.
- `pkgs/packages/clankpipe-action-worktree/CONTEXT.md` for the Git worktree action.

If any listed ADR directory is empty, proceed silently. Do not create new ADR folders just to satisfy layout; producer skills create them when decisions exist.

## Use the glossary vocabulary

When output names a domain concept in issue titles, refactor proposals, hypotheses, or tests, use terms from the relevant `CONTEXT.md`:

- workspace
- source
- item
- store
- action
- run
- watch

Do not drift to synonyms unless the glossary changes.

## Flag ADR conflicts

If output contradicts an existing ADR, surface it explicitly instead of silently overriding it.

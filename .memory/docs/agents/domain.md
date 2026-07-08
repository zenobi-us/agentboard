# Domain Docs

How engineering skills should consume this repo's domain documentation when exploring the codebase.

## Layout

This repo uses a **multi-context** domain-doc layout.

Read these before exploring code:

- `CONTEXT-MAP.md` at the repo root. It defines the monorepo contexts and points agents to the relevant `CONTEXT.md` files.
- `apps/cli/CONTEXT.md` when touching the Rust CLI, split AgentBoard crates, or documenting AgentBoard behavior.
- Context-specific `CONTEXT.md` files only when `CONTEXT-MAP.md` lists one for the touched app/package.
- `.memory/docs/adr/` for system-wide ADRs, if present.
- `.memory/docs/adr/<relativepath>/` for context-scoped ADRs, if present.

If any of these files or directories do not exist, proceed silently. Do not create ADR folders upfront; producer skills create them lazily when decisions exist.

## Use the glossary vocabulary

When output names a domain concept in issue titles, refactor proposals, hypotheses, or tests, use terms from the relevant `CONTEXT.md`:

- workspace
- source
- item
- store
- action

Do not drift to synonyms unless the glossary changes.

## Flag ADR conflicts

If output contradicts an existing ADR, surface it explicitly instead of silently overriding it.

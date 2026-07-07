# Domain Docs

How engineering skills should consume this repo's domain documentation when exploring the codebase.

## Layout

This repo uses a **multi-context** domain-doc layout.

Read these before exploring code:

- `CONTEXT-MAP.md` at the repo root, if present. It describes component boundaries and should point agents toward relevant contexts.
- `CONTEXT.md` at the repo root for current AgentBoard glossary and architecture constraints.
- Any context-specific `CONTEXT.md` relevant to the touched app/package, if added later.
- `.memory/docs/adr/` for system-wide ADRs, if present.
- `.memory/docs/adr/<relativepath>/` for context-scoped ADRs, if present.

If any of these files or directories do not exist, proceed silently. Do not create ADR folders upfront; producer skills create them lazily when decisions exist.

## Use the glossary vocabulary

When output names a domain concept in issue titles, refactor proposals, hypotheses, or tests, use terms from `CONTEXT.md`:

- workspace
- source
- item
- store
- action

Do not drift to synonyms unless the glossary changes.

## Flag ADR conflicts

If output contradicts an existing ADR, surface it explicitly instead of silently overriding it.

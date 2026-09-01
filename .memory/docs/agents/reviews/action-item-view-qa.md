# Action Item View QA

## Scope

Reviewed and implemented the action item view QA feedback.

## Changes

- Added `DefinitionGrid` and `DefinitionGridItem` for muted labels and aligned values.
- Reused the grid in item summaries and source details.
- Added Workspace, Source, Item, and Action breadcrumbs to the item views.
- Added the `actionItemKeymap` with `O` for opening an item.
- Added optional `open` metadata to configured Actions.
- Rendered `open` commands with the existing MiniJinja context.
- Added an async shell runner for open commands.

## Verification

- `bun test apps/cli/src/services/open-item.test.ts apps/cli/src/services/plugins.test.ts pkgs/crates/agentboard-core/src/config.test.ts`
- `bunx tsc --noEmit -p apps/cli/tsconfig.json`
- `bunx tsc --noEmit -p pkgs/crates/agentboard-core/tsconfig.json`
- `git diff --check`

All checks passed.

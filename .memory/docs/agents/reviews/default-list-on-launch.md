# Default list action on TUI launch

## Scope

The TUI starts the existing `list` run after workspace loading completes. Badge type initials also use a darker inset background for contrast.

## Changes

- `apps/cli/src/tui/services/app/machine.ts` sets `runRequest.mode` to `list` in the workspace load completion action.
- `apps/cli/src/tui/components/app/badge.tsx` renders the type initial inside a contrasting background.
- The existing `AppScreen` effect runs the list operation through the normal runtime path.

## Validation

- `git diff --check`: passed.
- `bun test ./apps/cli/src/cli/list.test.ts`: passed.
- `moon run agentboard:ts-typecheck`: passed.

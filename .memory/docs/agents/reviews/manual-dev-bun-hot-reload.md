# Review: Bun development task

## Scope

Move the `apps/cli/package.json` scripts into `apps/cli/moon.yml` and add a Bun hot-reload development task.

## Changes reviewed

- Removed the `scripts` block from `apps/cli/package.json`.
- Added direct Moon commands for development, type checking, building, testing, and starting the CLI.
- Added `agentboard:dev` with `bun --hot src/cli/index.ts`.
- Kept existing public `build`, `check`, `typecheck`, and `test` task aliases.

## Validation

- `moon run agentboard:ts-typecheck` passed.
- `moon run agentboard:ts-build` passed.
- `moon run agentboard:ts-test` passed: 91 tests.
- `package.json` parsed successfully.
- `git diff --check` passed.
- `moon query projects` listed `agentboard:dev`.

## Notes

The package has no npm/Bun `scripts` block now. Run development through Moon:

```sh
moon run agentboard:dev
```

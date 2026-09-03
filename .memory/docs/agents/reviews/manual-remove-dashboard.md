# Remove obsolete dashboard review

## Scope

- Remove the obsolete read-only `dashboard` command.
- Keep `readStoreViews` for `list` and runtime tests.
- Remove the stale TUI placeholder item.
- Update active CLI documentation references.

## Findings

None.

The dashboard command is no longer registered or built from source. The focused CLI test passes. The full ClankPipe test suite passes. Store view code remains in use.

Historical research and review records still use `Dashboard` as historical terminology. They do not describe the active command surface.

## Validation

- `bun test apps/cli/src/cli/commands.test.ts`: PASS; 9 tests, 30 assertions.
- `moon run clankpipe:ts-test`: PASS; 110 tests, 274 assertions.
- `moon run clankpipe:typecheck`: PASS.
- `moon run clankpipe:build`: PASS.
- `git diff --check`: PASS.

## Verdict

SUCCESS

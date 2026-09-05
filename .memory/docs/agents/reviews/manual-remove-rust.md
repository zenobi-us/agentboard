# Review: Remove Rust residue from ClankPipe

## Scope

Remove stale Rust artifacts and rename the TypeScript workspace directory from `pkgs/crates` to `pkgs/packages`.

## Changes

- Deleted stale Rust implementation research:
  - `.memory/docs/research/agentboard-cli-configuration-docs-audit-2026-07-19.md`
  - `.memory/docs/research/plugin-registry-interface-research-2026-07-20.md`
  - `.memory/docs/research/source-field-mapping-audit-2026-07-20.md`
- Removed Rust runtime wording from ADR 0010 and ADR 0012.
- Renamed `pkgs/crates` to `pkgs/packages`.
- Renamed matching scoped ADR directories.
- Updated Moon, Bun workspace, documentation, tests, and context references.
- Regenerated `bun.lock` with `bun install --lockfile-only`.

## Validation

- `bun install` passed.
- `moon run :typecheck --affected` passed.
- `moon run :test --affected --force` passed.
- `git diff --check` passed.
- No tracked Rust source or Cargo files remain.

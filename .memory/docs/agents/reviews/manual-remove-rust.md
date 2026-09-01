# Review: Remove Rust from AgentBoard

## Scope

Remove Rust source and Rust-only configuration from the monorepo. Keep the Bun and TypeScript implementation.

## Changes

- Deleted the root Cargo workspace and all tracked Rust source files.
- Deleted the Rust CLI integration tests and Rust build artifacts.
- Converted Moon projects to TypeScript or Bun tasks.
- Restored the TypeScript test task for `agentboard-action-run-cmd`.
- Removed Rust from proto, Moon, release, publish, and deployment configuration.
- Updated active docs and context files from Rust/crate wording to Bun/package wording.
- Kept the existing `pkgs/crates/*` paths to avoid an unrelated package-path migration.

## Validation

- `bun install --frozen-lockfile` passed.
- `moon run :typecheck --affected` passed.
- `moon run :test --affected --force` passed.
- `moon run agentboard:build --force` passed.
- `moon run agentboard-action-run-cmd:test --force` passed: 4 tests.
- `git diff --check` passed.

## Known unrelated failure

`moon run :build --affected --force` failed in the existing docs Waku/Vite build with `TypeError: entry.INTERNAL_runBuild is not a function`. The CLI build passed separately.

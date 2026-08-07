# CLI workspace and output plan

Status: implemented and verified

## Workspace commands

- Add `agentboard workspace list`.
- Keep `agentboard workspaces` as a compatibility alias.
- Add `agentboard workspace init <name>`.
- `workspace init` creates an empty named Workspace at the XDG AgentBoard config directory.
- The generated TOML contains no Sources and is ready for manual configuration.
- Initialization MUST refuse to overwrite an existing Workspace and MUST report its path.
- Do not add `workspace add`; its intended domain operation is unclear and no current need justifies it.

## Output contract

- Human progress MUST be written to stderr. Command results and machine-readable output remain on stdout.
- `doctor` and `run` MUST show concise progress by default.
- Global `-v` MUST enable detailed progress, including successful Action output.
- Global `-q` MUST suppress non-error progress.
- Failed Actions MUST show captured stdout/stderr without requiring `-v`.

## Colour

- Add global `--color auto|always|never` with `auto` as default.
- `auto` MUST colour only human output written to an interactive terminal.
- `NO_COLOR` MUST disable colour unless `--color always` explicitly overrides it.
- Structured or redirected output MUST NOT contain ANSI escapes by default.

## Diagnostic log file

- Add global `--log-file <path>`.
- The file MUST use append-only JSON Lines.
- Records SHOULD include timestamps, invocation/run identity, Workspace and Source identifiers, stage, outcome, counts, duration, and error chains where relevant.
- Records MUST contain metadata only.
- Records MUST NOT contain rendered Action inputs or captured Action stdout/stderr; those remain in Store Action attempts.

## Doctor

- `doctor` MUST run all independent checks instead of stopping after the first failure.
- It MUST report pass/fail for Workspace config, Store writability, Source reachability, and required commands.
- A reachable Source MUST report fetched Item count and configured limit.
- It MUST also report upstream available count when the Source API exposes one without fetching beyond the configured limit; otherwise it MUST say availability is unknown.
- Beneath each Source, it MUST report the number of configured Actions and one status line per Action.
- An Action is healthy only when its required inputs and known executable prerequisites are valid. Doctor MUST NOT render or execute Actions.
- It MUST exit non-zero when any check fails.

## Run and watch

- `run` SHOULD report Workspace start/end, Source collection outcomes and item counts, Action attempted/skipped/succeeded/failed counts, and duration.
- Watch Mode (`run --watch`) SHOULD additionally report cycle identity, cycle outcome, next scheduled run, and clean Ctrl-C shutdown.
- Detailed mode SHOULD report per-Item and per-Action progress.

## Documentation decision

- No `CONTEXT.md` glossary additions: these behaviors do not introduce new domain concepts.
- No ADR: decisions are reversible, unsurprising CLI policy, and do not meet ADR threshold.

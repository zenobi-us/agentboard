---
id: "TASK-001"
title: "Add a named greeting flag"
status: "ready"
queue: "agentboard-ready"
url: "https://example.invalid/tasks/TASK-001"
---

# Add a named greeting flag

Add `--name NAME` support to `greet.sh` while preserving the existing positional
name and default `World` behavior.

## Acceptance criteria

- `./greet.sh --name Ada` prints `Hello, Ada!`.
- `./greet.sh Grace` still prints `Hello, Grace!`.
- `./greet.sh` still prints `Hello, World!`.
- `./test.sh` covers all three cases and passes.
- `README.md` documents the new flag.

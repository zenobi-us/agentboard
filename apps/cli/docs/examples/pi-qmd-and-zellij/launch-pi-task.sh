#!/usr/bin/env bash
set -euo pipefail

task_id="${1:?task id is required}"
task_title="${2:?task title is required}"
: "${ZELLIJ_SESSION_NAME:?launch-pi-task.sh must run inside Zellij}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
task_file="$root/tasks/$task_id.md"
worktree="$PWD"

test -f "$task_file" || {
  echo "task file not found: $task_file" >&2
  exit 1
}

printf -v command 'pi --name %q %q' \
  "$task_id" \
  "Implement the task in @$task_file. Work only in the current worktree and run its tests before stopping."

zellij action new-tab --name "$task_id" --cwd "$worktree"
zellij action write-chars "$command"
zellij action write 13

printf 'launched %s: %s\n' "$task_id" "$task_title"

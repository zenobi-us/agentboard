#!/usr/bin/env bash
set -Eeuo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly ROOT
cd "$ROOT"

usage() {
  cat <<'EOF'
Usage: ./test.sh [task-NN]

Without a task, validate all demo HTML and CSS.
With a task, also verify that task's acceptance criterion.
EOF
}

fail() {
  printf 'test: %s\n' "$*" >&2
  exit 1
}

contains() {
  local file="$1"
  local text="$2"
  grep -Fq -- "$text" "$file" || fail "$file is missing: $text"
}

case "${1:-}" in
  -h|--help)
    usage
    exit 0
    ;;
  '')
    html_targets=(index.html pages/*.html)
    ;;
  task-0[1-9]|task-1[0-2])
    task="$1"
    html_targets=("pages/$task.html")
    ;;
  *)
    usage >&2
    exit 2
    ;;
esac

bunx --bun @linthtml/linthtml "${html_targets[@]}"

css_output="$(mktemp)"
trap 'rm -f "$css_output"' EXIT
bunx --bun lightningcss-cli styles.css -o "$css_output"
[[ -s "$css_output" ]] || fail 'Lightning CSS produced no output'

if [[ -n "${task:-}" ]]; then
  file="pages/$task.html"
  case "$task" in
    task-01)
      contains "$file" 'aria-labelledby="status-heading"'
      ;;
    task-02)
      contains "$file" 'data-copy-command="agentboard watch"'
      ;;
    task-03)
      first_child="$(awk '/<table>/{inside=1; next} inside && NF {sub(/^[[:space:]]+/, ""); print; exit}' "$file")"
      [[ "$first_child" == '<caption>Active AgentBoard worktrees</caption>' ]] ||
        fail "$file must place the required caption first inside the table"
      ;;
    task-04)
      contains "$file" 'role="status"'
      ;;
    task-05)
      contains "$file" 'aria-live="polite"'
      ;;
    task-06)
      contains "$file" '<label for="session-name">Session name</label>'
      ! grep -Fq -- '<span>Session name</span>' "$file" || fail "$file still contains the old session-name span"
      ;;
    task-07)
      contains "$file" '<dialog id="cleanup-confirmation">'
      contains "$file" '</dialog>'
      ! grep -Fq -- '<section id="cleanup-confirmation">' "$file" || fail "$file still uses a section"
      ;;
    task-08)
      contains "$file" 'aria-label="Active agents"'
      ;;
    task-09)
      contains "$file" '<progress value="2" max="12">'
      contains "$file" '2 of 12 tasks complete'
      ;;
    task-10)
      contains "$file" 'class="skip-link"'
      contains "$file" 'href="#main"'
      ;;
    task-11)
      contains "$file" '<noscript>Agent activity requires JavaScript.</noscript>'
      ;;
    task-12)
      contains "$file" 'name="description"'
      contains "$file" 'content="AgentBoard demo dashboard"'
      ;;
  esac
fi

printf 'Passed HTML and CSS validation%s.\n' "${task:+ for $task}"

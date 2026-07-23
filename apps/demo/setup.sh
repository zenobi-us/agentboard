#!/usr/bin/env bash
set -Eeuo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly ROOT
readonly CONFIG="$ROOT/.agentboard.toml"
readonly LABELS=(
  'agentboard:ready-for-agent|2f81f7|Ready for an implementation agent'
  'agentboard:changes-requested|d93f0b|Review requested implementation changes'
  'agentboard:ready-for-review|8250df|Ready for a review agent'
  'agentboard:review-complete|1f883d|Agent review passed'
  'agentboard:cleanup-approved|6f42c1|Worktree cleanup approved'
)

usage() {
  cat <<'EOF'
Usage: ./setup.sh

Prepare a standalone AgentBoard demo repository:
  - configure its GitHub repository query
  - create AgentBoard workflow labels
  - create GitHub issues from .issues/*.json

Run this only after copying apps/demo into a new, committed GitHub repository.
EOF
}

fail() {
  printf 'setup: %s\n' "$*" >&2
  exit 1
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || fail "required command not found: $1"
}

case "${1:-}" in
  -h|--help)
    usage
    exit 0
    ;;
  '') ;;
  *)
    usage >&2
    exit 2
    ;;
esac

for command in git gh bun agentboard pi xdg-terminal-exec; do
  require_command "$command"
done

gh auth status >/dev/null 2>&1 || fail 'GitHub CLI is not authenticated; run: gh auth login'

repo_root="$(git -C "$ROOT" rev-parse --show-toplevel 2>/dev/null)" ||
  fail 'demo directory is not inside a Git repository'
[[ "$repo_root" == "$ROOT" ]] ||
  fail 'copy apps/demo into a standalone repository before running setup.sh'
[[ -z "$(git -C "$ROOT" status --porcelain)" ]] ||
  fail 'working tree must be clean before setup'

cd "$ROOT"
repo="$(gh repo view --json nameWithOwner --jq .nameWithOwner 2>/dev/null)" ||
  fail 'current repository has no GitHub remote'

if grep -Fq '__GITHUB_REPOSITORY__' "$CONFIG"; then
  sed -i "s|__GITHUB_REPOSITORY__|$repo|g" "$CONFIG"
  git add .agentboard.toml
  git commit -m 'chore: configure AgentBoard demo repository'
  git push
fi

for label in "${LABELS[@]}"; do
  IFS='|' read -r name color description <<<"$label"
  gh label create "$name" --repo "$repo" --color "$color" --description "$description" --force >/dev/null
done

existing_titles="$(gh issue list --repo "$repo" --state all --limit 100 --json title --jq '.[].title')"
shopt -s nullglob
issue_files=(.issues/*.json)
((${#issue_files[@]} > 0)) || fail 'no issue JSON files found in .issues/'

for task in "${issue_files[@]}"; do
  title="$(bun -e 'console.log((await Bun.file(process.argv.at(-1)).json()).title)' "$task")"
  if grep -Fxq -- "$title" <<<"$existing_titles"; then
    printf 'Skipped existing issue: %s\n' "$title" >&2
    continue
  fi

  number="$(gh api --method POST "repos/$repo/issues" --input "$task" --jq .number)"
  printf 'Created issue #%s: %s\n' "$number" "$title"
  existing_titles+=$'\n'"$title"
done

cat <<EOF

Demo ready: https://github.com/$repo

Start AgentBoard:
  cd $ROOT
  agentboard watch .agentboard.toml --interval 15s

Queue an issue from another terminal:
  gh issue edit <number> --repo $repo --add-label agentboard:ready-for-agent
EOF

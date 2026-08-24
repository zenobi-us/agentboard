#!/usr/bin/env bash
set -euo pipefail

usage() {
  printf 'Usage: %s <implement|review> <issue> <item-slug>\n' "$0" >&2
}

fail() {
  printf 'agentboard runner: %s\n' "$*" >&2
  exit 1
}

[ "$#" -eq 3 ] || { usage; exit 2; }
role=$1
issue=$2
item_slug=$3

case "$role" in
  implement|review) ;;
  *) fail "unsupported role: $role" ;;
esac
case "$issue:$item_slug" in
  *[!A-Za-z0-9._:-]*) fail "invalid issue or item slug" ;;
esac

[ "${HERDR_ENV:-}" = 1 ] || fail 'HERDR_ENV=1 is required'
for command in herdr jq wt pi; do
  command -v "$command" >/dev/null 2>&1 || fail "required command not found: $command"
done

repo=$(git rev-parse --show-toplevel)
branch="agentboard/$item_slug"
parent_workspace=${HERDR_WORKSPACE_ID:-}
[ -n "$parent_workspace" ] || fail 'HERDR_WORKSPACE_ID is missing'

worktrees=$(wt -C "$repo" list --format json)
worktree_path=$(jq -r --arg branch "$branch" \
  'first(.[] | select(.branch == $branch) | .path) // empty' <<<"$worktrees")

if [ -n "$worktree_path" ]; then
  switch_json=$(wt -C "$repo" switch "$branch" --no-cd --format json)
else
  switch_json=$(wt -C "$repo" switch --create "$branch" --base main --no-cd --format json)
fi

worktree_path=$(jq -er '(.worktree.path // .path // .worktree_path)' <<<"$switch_json")
worktree_path=$(cd -- "$worktree_path" && pwd -P)

workspace_json=$(herdr worktree list --workspace "$parent_workspace")
open_workspace=$(jq -r --arg path "$worktree_path" \
  'first(.result.worktrees[]? | select(.path == $path) | .open_workspace_id // empty)' \
  <<<"$workspace_json")

if [ -n "$open_workspace" ]; then
  tab_json=$(herdr tab create \
    --workspace "$open_workspace" \
    --cwd "$worktree_path" \
    --label "$role-$issue" \
    --no-focus)
  pane_id=$(jq -er '.result.root_pane.pane_id' <<<"$tab_json")
else
  open_json=$(herdr worktree open \
    --workspace "$parent_workspace" \
    --path "$worktree_path" \
    --label "issue-$issue" \
    --no-focus)
  pane_id=$(jq -er '.result.root_pane.pane_id' <<<"$open_json")
fi

artifact_root="${XDG_DATA_HOME:-$HOME/.local/share}/agentboard/$AGENTBOARD_WORKSPACE_ID/exports/$AGENTBOARD_SOURCE_ID"
session_dir="$artifact_root/sessions"
session_id="$AGENTBOARD_SOURCE_ID-$issue"
mkdir -p "$session_dir"

agent_name=$(printf '%s-%s' "$role" "$issue" | tr '[:upper:]' '[:lower:]' | tr -c 'a-z0-9_-' '-')
herdr agent start "$agent_name" \
  --kind pi \
  --pane "$pane_id" \
  --timeout 300000 \
  -- \
  --session-dir "$session_dir" \
  --session-id "$session_id" \
  "/$role $issue"
wait_json=$(herdr agent wait "$agent_name" \
  --until done \
  --until blocked \
  --timeout 86400000)
wait_status=$(jq -r '.result.status // .result.agent_status // empty' <<<"$wait_json")
[ "$wait_status" = done ] || fail "agent stopped in state: ${wait_status:-unknown}"

session_file=$(find "$session_dir" -maxdepth 1 -type f \
  -name "*_${session_id}.jsonl" -print -quit)
if [ -n "$session_file" ]; then
  pi --export "$session_file" "$artifact_root/$session_id.html"
fi

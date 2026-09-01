#!/usr/bin/env bash
set -euo pipefail

usage() {
	cat <<'EOF'
Usage:
  ./launch-agent.sh launch <implement|review> <issue>
  ./launch-agent.sh cleanup <issue>

Environment:
  CLANKPIPE_LAUNCHER=auto|herdr|zellij|gnome-terminal|xterm|konsole|kitty|alacritty|wezterm
  CLANKPIPE_LAUNCH_MODE=workspace|issue-pane|pane|tab
EOF
}

fail() {
	printf 'CLANKPIPE LAUNCHER: ❌ %s\n' "$*" >&2
	exit 1
}

require_command() {
	command -v "$1" >/dev/null 2>&1 || fail "required command not found: $1"
}

validate_issue() {
	case "$1" in
	'' | *[!A-Za-z0-9._-]*) fail "invalid issue reference: $1" ;;
	esac
}

resolve_launcher() {
	local requested=${CLANKPIPE_LAUNCHER:-auto}

	if [ "$requested" != auto ]; then
		printf '%s\n' "$requested"
		return
	fi

	if [ "${HERDR_ENV:-}" = 1 ] && command -v herdr >/dev/null 2>&1; then
		printf 'herdr\n'
		return
	fi
	if [ -n "${ZELLIJ:-}" ] && command -v zellij >/dev/null 2>&1; then
		printf 'zellij\n'
		return
	fi

	for launcher in gnome-terminal xterm konsole kitty alacritty wezterm; do
		if command -v "$launcher" >/dev/null 2>&1; then
			printf '%s\n' "$launcher"
			return
		fi
	done

	fail 'no supported launcher found; set CLANKPIPE_LAUNCHER or install one'
}

launch_zellij() {
	local role=$1
	local issue=$2
	local name="${role}-${issue}"
	local tab_name="issue-${issue}"
	local mode=${CLANKPIPE_LAUNCH_MODE:-issue-pane}

	require_command zellij
	case "$mode" in
	issue-pane)
		require_command jq
		local tab_id
		tab_id=$(zellij action list-tabs --json | jq -r --arg name "$tab_name" 'first(.[] | select(.name == $name) | .tab_id) // empty')
		if [ -n "$tab_id" ]; then
			zellij action new-pane --tab-id "$tab_id" --cwd "$PWD" --name "$name" -- pi "/$role $issue"
		else
			zellij action new-tab --cwd "$PWD" --name "$tab_name" -- pi "/$role $issue"
		fi
		;;
	pane)
		zellij action new-pane --cwd "$PWD" --name "$name" -- pi "/$role $issue"
		;;
	tab)
		zellij action new-tab --cwd "$PWD" --name "$tab_name" -- pi "/$role $issue"
		;;
	*)
		fail "unsupported Zellij launch mode: $mode"
		;;
	esac
}

canonical_path() {
	(
		cd -- "$1" >/dev/null 2>&1 &&
		pwd -P
	)
}

worktree_workspace_for_path() {
	local cwd=$1
	local parent_workspace_id=${HERDR_WORKSPACE_ID:-}

	[ -n "$parent_workspace_id" ] || return 1
	herdr worktree list --workspace "$parent_workspace_id" |
		jq -r --arg cwd "$cwd" '
			first(.result.worktrees[]? |
				select(.path == $cwd) |
				.open_workspace_id // empty)'
}

launch_herdr_workspace() {
	local role=$1
	local issue=$2
	local name="${role}-${issue}"
	local cwd
	local parent_workspace_id=${HERDR_WORKSPACE_ID:-}
	local workspace_id
	local worktree_json
	local tab_json
	local pane_id
	local command

	[ -n "$parent_workspace_id" ] || fail 'Herdr workspace context is missing'
	cwd=$(canonical_path "$PWD") || fail "worktree path does not exist: $PWD"
	workspace_id=$(worktree_workspace_for_path "$cwd")

	if [ -n "$workspace_id" ]; then
		tab_json=$(herdr tab create \
			--workspace "$workspace_id" \
			--cwd "$cwd" \
			--label "$name" \
			--no-focus)
		pane_id=$(jq -er '.result.root_pane.pane_id' <<<"$tab_json")
	elif [ "$role" = implement ]; then
		# ClankPipe/Worktrunk owns Git worktree creation. Herdr only opens it.
		worktree_json=$(herdr worktree open \
			--workspace "$parent_workspace_id" \
			--path "$cwd" \
			--label "issue-${issue}" \
			--no-focus)
		pane_id=$(jq -er '.result.root_pane.pane_id' <<<"$worktree_json")
	else
		fail "issue workspace is not open; launch implement first for issue $issue"
	fi

	printf -v command 'cd %q && exec pi %q' "$cwd" "/$role $issue"
	herdr pane run "$pane_id" "$command"
}

launch_herdr_pane() {
	local role=$1
	local issue=$2
	local pane_json
	local pane_id
	local command

	pane_json=$(herdr pane split --current --direction right --cwd "$PWD" --no-focus)
	pane_id=$(jq -er '.result.pane.pane_id' <<<"$pane_json")
	printf -v command 'cd %q && exec pi %q' "$PWD" "/$role $issue"
	herdr pane run "$pane_id" "$command"
}

launch_herdr() {
	local role=$1
	local issue=$2
	local mode=${CLANKPIPE_LAUNCH_MODE:-workspace}

	[ "${HERDR_ENV:-}" = 1 ] || fail 'Herdr launcher requires HERDR_ENV=1'
	require_command herdr
	require_command jq

	case "$mode" in
	workspace) launch_herdr_workspace "$role" "$issue" ;;
	issue-pane | pane) launch_herdr_pane "$role" "$issue" ;;
	*) fail 'Herdr launcher supports workspace, issue-pane, and pane modes' ;;
	esac
}

launch_terminal() {
	local launcher=$1
	local role=$2
	local issue=$3
	local name="${role}-${issue}"
	local prompt="/$role $issue"

	case "$launcher" in
	gnome-terminal)
		require_command gnome-terminal
		# The positional argument expands in the child shell.
		# shellcheck disable=SC2016
		gnome-terminal --title="$name" --working-directory="$PWD" -- bash -lc 'exec pi "$1"' bash "$prompt"
		;;
	xterm)
		require_command xterm
		# The positional arguments expand in the child shell.
		# shellcheck disable=SC2016
		xterm -T "$name" -e bash -lc 'cd "$1" && exec pi "$2"' bash "$PWD" "$prompt"
		;;
	konsole)
		require_command konsole
		konsole --new-window --workdir "$PWD" --title "$name" -e pi "$prompt"
		;;
	kitty)
		require_command kitty
		kitty --directory "$PWD" --title "$name" pi "$prompt"
		;;
	alacritty)
		require_command alacritty
		alacritty --working-directory "$PWD" --title "$name" -e pi "$prompt"
		;;
	wezterm)
		require_command wezterm
		wezterm start --cwd "$PWD" -- pi "$prompt"
		;;
	*)
		fail "unsupported launcher: $launcher"
		;;
	esac
}

close_zellij_tab() {
	local issue=$1
	local tab_name="issue-${issue}"
	local tab_id

	[ "${CLANKPIPE_LAUNCHER:-auto}" = zellij ] ||
		[ "${CLANKPIPE_LAUNCHER:-auto}" = auto ] && [ -n "${ZELLIJ:-}" ] || return 0
	command -v zellij >/dev/null 2>&1 || return 0
	require_command jq
	tab_id=$(zellij action list-tabs --json | jq -r --arg name "$tab_name" 'first(.[] | select(.name == $name) | .tab_id) // empty')
	[ -n "$tab_id" ] && zellij action close-tab-by-id "$tab_id"
}

launch_task() {
	local role=$1
	local issue=$2
	local launcher

	case "$role" in
	implement | review) ;;
	*) fail "unsupported task role: $role" ;;
	esac
	validate_issue "$issue"
	launcher=$(resolve_launcher)

	case "$launcher" in
	herdr) launch_herdr "$role" "$issue" ;;
	zellij) launch_zellij "$role" "$issue" ;;
	*) launch_terminal "$launcher" "$role" "$issue" ;;
	esac
}

cleanup_task() {
	local issue=$1
	validate_issue "$issue"
	close_zellij_tab "$issue"
	git worktree remove "worktrees/issue-${issue}" --force
}

case "${1:-}" in
-h | --help)
	usage
	;;
launch)
	[ "$#" -eq 3 ] || { usage >&2; exit 2; }
	launch_task "$2" "$3"
	;;
cleanup)
	[ "$#" -eq 2 ] || { usage >&2; exit 2; }
	cleanup_task "$2"
	;;
*)
	usage >&2
	exit 2
	;;
esac

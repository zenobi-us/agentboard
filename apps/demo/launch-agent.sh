#!/usr/bin/env bash
set -euo pipefail

usage() {
	cat <<'EOF'
Usage:
  ./launch-agent.sh launch <implement|review> <issue>
  ./launch-agent.sh cleanup <issue>

Environment:
  AGENTBOARD_LAUNCHER=auto|herdr|zellij|gnome-terminal|xterm|konsole|kitty|alacritty|wezterm
  AGENTBOARD_LAUNCH_MODE=issue-pane|pane|tab
EOF
}

fail() {
	printf 'AGENTBOARD LAUNCHER: ❌ %s\n' "$*" >&2
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
	local requested=${AGENTBOARD_LAUNCHER:-auto}

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

	fail 'no supported launcher found; set AGENTBOARD_LAUNCHER or install one'
}

launch_zellij() {
	local role=$1
	local issue=$2
	local name="${role}-${issue}"
	local tab_name="issue-${issue}"
	local mode=${AGENTBOARD_LAUNCH_MODE:-issue-pane}

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

launch_herdr() {
	local role=$1
	local issue=$2
	local mode=${AGENTBOARD_LAUNCH_MODE:-issue-pane}
	local pane_json
	local pane_id
	local command

	[ "${HERDR_ENV:-}" = 1 ] || fail 'Herdr launcher requires HERDR_ENV=1'
	[ "$mode" = issue-pane ] || [ "$mode" = pane ] ||
		fail 'Herdr launcher supports pane mode only'
	require_command herdr
	require_command jq

	pane_json=$(herdr pane split --current --direction right --cwd "$PWD" --no-focus)
	pane_id=$(jq -er '.result.pane.pane_id' <<<"$pane_json")
	printf -v command 'cd %q && exec pi %q' "$PWD" "/$role $issue"
	herdr pane run "$pane_id" "$command"
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

	[ "${AGENTBOARD_LAUNCHER:-auto}" = zellij ] ||
		[ "${AGENTBOARD_LAUNCHER:-auto}" = auto ] && [ -n "${ZELLIJ:-}" ] || return 0
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

#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$root/demo-repo"

for command in agentboard git pi qmd zellij; do
	command -v "$command" >/dev/null || {
		echo "$command is required" >&2
		exit 1
	}
done

# TODO: make demo layout referene the http hosted copies of these plugins instead of requiring them to be installed locally
for plugin in zellij-agent-threads.wasm zellij-tabbar.wasm; do
	test -f "$HOME/.config/zellij/plugins/$plugin" || {
		echo "missing ~/.config/zellij/plugins/$plugin" >&2
		exit 1
	}
done

if [ "$(git -C "$repo" rev-parse --show-toplevel 2>/dev/null || true)" != "$repo" ]; then
	git -C "$repo" init -b main
	git -C "$repo" config user.name "AgentBoard example"
	git -C "$repo" config user.email "agentboard@example.invalid"
	git -C "$repo" add README.md greet.sh test.sh
	git -C "$repo" commit -m "chore: initialize greeting demo"
fi

cd "$root"
qmd update
qmd embed

echo "Example ready. Start Zellij with:"
echo "  zellij --session agentboard-demo --new-session-with-layout $root/zellij-layout.kdl"

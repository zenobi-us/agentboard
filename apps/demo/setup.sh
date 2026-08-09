#!/bin/sh
set -eu

readonly SOURCE_ARCHIVE='https://github.com/zenobi-us/agentboard/archive/refs/heads/main.tar.gz'

readonly LABELS='[
{ "label": "agentboard:ready-for-agent", "colour": "2f81f7", "description": "Ready for an implementation agent" },
{ "label": "agentboard:in-progress", "colour": "1d76db", "description": "Implementation agent owns the issue" },
{ "label": "agentboard:changes-requested", "colour": "d93f0b", "description": "Review requested implementation changes" },
{ "label": "agentboard:ready-for-review", "colour": "8250df", "description": "Ready for a review agent" },
{ "label": "agentboard:review-in-progress", "colour": "5319e7", "description": "Review agent owns the issue" },
{ "label": "agentboard:review-complete", "colour": "1f883d", "description": "Agent review passed" },
{ "label": "agentboard:cleanup-approved", "colour": "6f42c1", "description": "Worktree cleanup approved" }
]'

tmp=''

usage() {
	cat <<'EOF'
Usage:
  curl https://raw.githubusercontent.com/zenobi-us/agentboard/refs/heads/main/apps/demo/setup.sh | sh

Set REPO=owner/name to skip the repository prompt.
The private repository is cloned into ./<name>.
EOF
}

action() {
	printf 'AGENTBOARD DEMO SETUP: ℹ️ %s\n' "$*" >&2
}

fail() {
	printf 'AGENTBOARD DEMO SETUP: ❌ %s\n' "$*" >&2
	exit 1
}

success() {
	printf 'AGENTBOARD DEMO SETUP: ✅ %s\n' "$*" >&2
}

confirm_replacement() {
	[ -r /dev/tty ] || fail "$1 exists; interactive confirmation is required to replace it"
	printf '%s exists. %s? [y/N] ' "$1" "$2" >/dev/tty
	answer=''
	IFS= read -r answer </dev/tty || fail 'could not read confirmation'
	case "$answer" in
	y | Y | yes | YES | Yes) ;;
	*) fail 'setup cancelled; existing resources were left unchanged' ;;
	esac
}

require_command() {
	command -v "$1" >/dev/null 2>&1 || fail "required command not found: $1"
}

cleanup() {
	if [ -n "$tmp" ] && [ -d "$tmp" ]; then
		rm -rf "$tmp"
	fi
}

trap cleanup 0 HUP INT TERM

case "${1:-}" in
-h | --help)
	usage
	exit 0
	;;
'') ;;
*)
	usage >&2
	exit 2
	;;
esac

for command in curl git gh jq bun agentboard pi tar zellij; do
	require_command "$command"
done

gh auth status >/dev/null 2>&1 || fail 'GitHub CLI is not authenticated; run: gh auth login'

if [ -z "${REPO:-}" ]; then
	[ -r /dev/tty ] || fail 'REPO is required when no interactive terminal is available'
	printf 'GitHub repository (owner/name): ' >/dev/tty
	IFS= read -r REPO </dev/tty || fail 'could not read repository name'
fi

owner=${REPO%%/*}
name=${REPO#*/}
[ -n "$owner" ] && [ -n "$name" ] && [ "$owner" != "$name" ] ||
	fail 'REPO must use owner/name format'
case "$name" in
*/* | . | ..) fail 'REPO must use owner/name format' ;;
esac

target=$PWD/$name
replace_target=false
replace_repo=false
if [ -e "$target" ]; then
	confirm_replacement "Local path $target" 'Delete and replace it'
	replace_target=true
fi
if gh repo view "$REPO" >/dev/null 2>&1; then
	confirm_replacement "GitHub repository $REPO" 'Permanently delete all issues and replace its commit history with the new demo'
	replace_repo=true
fi

action "Setting up AgentBoard demo in $REPO (local clone: $target)"
tmp=$(mktemp -d)
archive=$tmp/agentboard.tar.gz
source_dir=$tmp/source
mkdir "$source_dir"

action 'Downloading AgentBoard demo...'
curl -fsSL "$SOURCE_ARCHIVE" -o "$archive"
tar -xzf "$archive" -C "$source_dir"

source_root=''
for directory in "$source_dir"/*; do
	source_root=$directory
	break
done
[ -d "$source_root/apps/demo" ] || fail 'downloaded archive does not contain apps/demo'

if [ "$replace_target" = true ]; then
	action "Deleting existing local path $target..."
	rm -rf "$target"
fi
if [ "$replace_repo" = false ]; then
	action "Creating private repository $REPO..."
	gh repo create "$REPO" --private
fi

action "Cloning repository $REPO..."
gh repo clone "$REPO" "$target"
if [ "$replace_repo" = true ]; then
	action 'Preparing clean demo commit history...'
	git -C "$target" checkout --orphan agentboard-demo-replacement
	git -C "$target" rm -rf --ignore-unmatch .
fi
cp -R "$source_root/apps/demo/." "$target/"

sed "s|__GITHUB_REPOSITORY__|$REPO|g" "$target/.agentboard.toml" >"$tmp/agentboard.toml"
mv "$tmp/agentboard.toml" "$target/.agentboard.toml"

action 'Installing dependencies...'
(
	cd "$target"
	bun install
)

action 'Preparing demo commit...'
git -C "$target" add .
git -C "$target" commit -m 'chore: initialize AgentBoard demo'
git -C "$target" branch -M main
if [ "$replace_repo" = true ]; then
	git -C "$target" push -u origin HEAD --force-with-lease
else
	git -C "$target" push -u origin HEAD
fi

action 'Configuring repository settings...'
gh api --method PATCH "repos/$REPO" --input - >/dev/null <<'EOF'
{
  "default_branch": "main",
  "allow_merge_commit": false,
  "allow_rebase_merge": false,
  "allow_squash_merge": true,
  "delete_branch_on_merge": true
}
EOF

action 'Configuring labels...'
printf '%s\n' "$LABELS" |
	jq -r '.[] | [.label, .colour, .description] | @tsv' |
	while IFS="$(printf '\t')" read -r label colour description; do
		gh label create "$label" --repo "$REPO" --color "$colour" --description "$description" --force >/dev/null
	done

action 'Deleting existing issues...'
gh issue list --repo "$REPO" --state all --limit 100000 --json id,number >"$tmp/issues.json"
jq -r '.[] | [.id, .number] | @tsv' "$tmp/issues.json" >"$tmp/issues.tsv"
while IFS="$(printf '\t')" read -r issue_id issue_number; do
	[ -n "$issue_id" ] || continue
	action "Permanently deleting issue #$issue_number..."
	gh api graphql \
		-f query="mutation(\$id: ID!) { deleteIssue(input: {issueId: \$id}) { clientMutationId } }" \
		-f id="$issue_id" >/dev/null
done <"$tmp/issues.tsv"

action 'Creating demo issues...'
set -- "$target"/.issues/*.json
[ -f "$1" ] || fail 'downloaded demo contains no issue JSON files'
for task; do
	title=$(bun -e 'console.log((await Bun.file(process.argv.at(-1)).json()).title)' "$task")
	number=$(gh api --method POST "repos/$REPO/issues" --input "$task" --jq .number)
	printf 'Created issue #%s: %s\n' "$number" "$title"
done

success 'Demo setup complete!'

cat <<EOF

Demo ready: https://github.com/$REPO
Local clone: $target

Start AgentBoard from the demo repository:
  cd $target
  agentboard run .agentboard.toml --watch --interval 15s

The launcher auto-detects Herdr, Zellij, and supported desktop terminals.
Override it with AGENTBOARD_LAUNCHER=gnome-terminal, xterm, or another supported launcher.
For Zellij, set AGENTBOARD_LAUNCH_MODE=tab to use a new tab instead of the default issue pane.

Queue an issue from another terminal:
  gh issue edit <number> --repo $REPO --add-label agentboard:ready-for-agent

After review passes, confirm that the PR is merged and the issue is closed, then approve cleanup:
  gh issue edit <number> --repo $REPO --add-label agentboard:cleanup-approved

If an agent exits before handing off, remove its in-progress label and restore the matching ready label.
EOF

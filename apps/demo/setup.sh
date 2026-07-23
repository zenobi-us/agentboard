#!/bin/sh
set -eu

readonly SOURCE_ARCHIVE='https://github.com/zenobi-us/agentboard/archive/refs/heads/main.tar.gz'
readonly LABELS='agentboard:ready-for-agent|2f81f7|Ready for an implementation agent
agentboard:changes-requested|d93f0b|Review requested implementation changes
agentboard:ready-for-review|8250df|Ready for a review agent
agentboard:review-complete|1f883d|Agent review passed
agentboard:cleanup-approved|6f42c1|Worktree cleanup approved'

tmp=''

usage() {
  cat <<'EOF'
Usage:
  curl https://raw.githubusercontent.com/zenobi-us/agentboard/refs/heads/main/apps/demo/setup.sh | sh

Set REPO=owner/name to skip the repository prompt.
The private repository is cloned into ./<name>.
EOF
}

fail() {
  printf 'setup: %s\n' "$*" >&2
  exit 1
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

for command in curl git gh bun npm npx agentboard pi tar; do
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
  */*) fail 'REPO must use owner/name format' ;;
esac

target=$PWD/$name
[ ! -e "$target" ] || fail "destination already exists: $target"
if gh repo view "$REPO" >/dev/null 2>&1; then
  fail "GitHub repository already exists: $REPO"
fi

tmp=$(mktemp -d)
archive=$tmp/agentboard.tar.gz
source_dir=$tmp/source
mkdir "$source_dir"

printf 'Downloading AgentBoard demo...\n' >&2
curl -fsSL "$SOURCE_ARCHIVE" -o "$archive"
tar -xzf "$archive" -C "$source_dir"

source_root=''
for directory in "$source_dir"/*; do
  source_root=$directory
  break
done
[ -d "$source_root/apps/demo" ] || fail 'downloaded archive does not contain apps/demo'

printf 'Creating private repository %s...\n' "$REPO" >&2
gh repo create "$REPO" --private
gh repo clone "$REPO" "$target"
cp -R "$source_root/apps/demo/." "$target/"

sed "s|__GITHUB_REPOSITORY__|$REPO|g" "$target/.agentboard.toml" >"$tmp/agentboard.toml"
mv "$tmp/agentboard.toml" "$target/.agentboard.toml"

cat >"$target/package.json" <<'EOF'
{
  "name": "agentboard-demo",
  "private": true,
  "type": "module",
  "scripts": {
    "lint": "eslint .",
    "prepare": "husky"
  },
  "lint-staged": {
    "*.html": "eslint",
    "*.css": "eslint"
  }
}
EOF

cat >"$target/eslint.config.js" <<'EOF'
import { defineConfig } from "eslint/config";
import css from "@eslint/css";
import html from "@html-eslint/eslint-plugin";

export default defineConfig([
  {
    files: ["**/*.html"],
    plugins: { html },
    language: "html/html",
    rules: {
      "html/no-duplicate-class": "error",
      "html/require-img-alt": "error",
    },
  },
  {
    files: ["**/*.css"],
    plugins: { css },
    language: "css/css",
    rules: {
      "css/no-duplicate-imports": "error",
      "css/no-empty-blocks": "error",
      "css/no-invalid-at-rules": "error",
      "css/no-invalid-properties": "error",
    },
  },
]);
EOF

(
  cd "$target"
  npm install --save-dev eslint @eslint/css @html-eslint/parser @html-eslint/eslint-plugin husky lint-staged
  mkdir -p .husky
  printf '%s\n' 'npx lint-staged' >.husky/pre-commit
  chmod +x .husky/pre-commit
  npx eslint '**/*.html' '**/*.css'
)

git -C "$target" add .
git -C "$target" commit -m 'chore: initialize AgentBoard demo'
git -C "$target" push -u origin HEAD

printf '%s\n' "$LABELS" | while IFS='|' read -r label color description; do
  gh label create "$label" --repo "$REPO" --color "$color" --description "$description" --force >/dev/null
done

set -- "$target"/.issues/*.json
[ -f "$1" ] || fail 'downloaded demo contains no issue JSON files'
for task do
  title=$(bun -e 'console.log((await Bun.file(process.argv.at(-1)).json()).title)' "$task")
  number=$(gh api --method POST "repos/$REPO/issues" --input "$task" --jq .number)
  printf 'Created issue #%s: %s\n' "$number" "$title"
done

cat <<EOF

Demo ready: https://github.com/$REPO
Local clone: $target

Start AgentBoard:
  cd $target
  agentboard watch .agentboard.toml --interval 15s

Queue an issue from another terminal:
  gh issue edit <number> --repo $REPO --add-label agentboard:ready-for-agent
EOF

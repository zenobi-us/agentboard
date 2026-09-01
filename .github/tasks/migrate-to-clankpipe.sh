#!/usr/bin/env bash
set -euo pipefail

old_repo=${OLD_REPO:-zenobi-us/agentboard}
new_repo=${NEW_REPO:-zenobi-us/clankpipe}
apply=false
[[ ${1:-} == --apply ]] && apply=true

labels=(ready-for-agent implementing changes-requested ready-for-review reviewing review-complete cleanup-approved)

run() {
  if $apply; then
    "$@"
  else
    printf '+%q' "$1"; shift; printf ' %q' "$@"; printf '\n'
  fi
}

printf 'Repository: %s -> %s\n' "$old_repo" "$new_repo"
printf 'Mode: %s\n' "$($apply && echo apply || echo dry-run)"

for label in "${labels[@]}"; do
  old_label="agentboard:${label}"
  [[ $label == ready-for-agent ]] && old_label="ready-for-agent"
  new_label="clankpipe:${label}"
  if $apply; then
    gh label create "$new_label" --repo "$new_repo" --color 5319e7 --force >/dev/null
  else
    printf '+ gh label create %q --repo %q --color 5319e7 --force\n' "$new_label" "$new_repo"
  fi

done

issues=$(gh issue list --repo "$new_repo" --state all --limit 1000 --json number,labels)
while IFS=$'\t' read -r number old_label; do
  [[ -n $number ]] || continue
  new_label="clankpipe:${old_label#agentboard:}"
  [[ $old_label == ready-for-agent ]] && new_label="clankpipe:ready-for-agent"
  if $apply; then
    gh issue edit "$number" --repo "$new_repo" --add-label "$new_label" >/dev/null
    gh issue edit "$number" --repo "$new_repo" --remove-label "$old_label" >/dev/null
  else
    printf '+ gh issue edit %q --repo %q --add-label %q\n' "$number" "$new_repo" "$new_label"
    printf '+ gh issue edit %q --repo %q --remove-label %q\n' "$number" "$new_repo" "$old_label"
  fi
done < <(jq -r '.[] | .number as $number | .labels[].name | select(startswith("agentboard:") or . == "ready-for-agent") | [$number, .] | @tsv' <<<"$issues")

if $apply; then
  git remote set-url origin "git@github.com:${new_repo}.git"
else
  printf '+ git remote set-url origin %q\n' "git@github.com:${new_repo}.git"
fi

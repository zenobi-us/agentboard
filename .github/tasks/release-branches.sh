#!/usr/bin/env bash

set -euo pipefail

require_arg() {
  local value="${1:-}"
  local name="${2:-arg}"

  if [[ -z "${value}" ]]; then
    echo "Missing ${name} argument" >&2
    exit 1
  fi
}

validate_branch_name() {
  local branch="${1:-}"
  local pattern="${2:-}"

  require_arg "${branch}" "branch name"
  require_arg "${pattern}" "branch pattern"

  if [[ ! "${branch}" =~ ${pattern} ]]; then
    echo "Branch name '${branch}' does not match pattern '${pattern}'" >&2
    exit 1
  fi
}

resolve_commit() {
  local ref="${1}"

  if ! git rev-parse --verify "${ref}^{commit}" 2>/dev/null; then
    echo "Unknown commit or branch: ${ref}" >&2
    exit 1
  fi
}

resolve_root_commit() {
  local release_branch="${1}"
  local main_branch="${2}"
  local release_commit main_commit

  release_commit="$(resolve_commit "${release_branch}")"
  main_commit="$(resolve_commit "${main_branch}")"

  if ! git merge-base "${release_commit}" "${main_commit}"; then
    echo "No shared commit between '${release_branch}' and '${main_branch}'" >&2
    exit 1
  fi
}

cmd_get_release_branch_commits() {
  local release_branch="${1}"
  local main_branch="${2:-main}"
  local release_commit root_commit

  release_commit="$(resolve_commit "${release_branch}")"
  root_commit="$(resolve_root_commit "${release_branch}" "${main_branch}")"

  printf '%s\n' "${root_commit}"
  git rev-list --reverse --first-parent "${root_commit}..${release_commit}"
}

cmd_is_root_commit() {
  local commit="${1}"
  local release_branch="${2}"
  local main_branch="${3:-main}"
  local resolved_commit root_commit

  resolved_commit="$(resolve_commit "${commit}")"
  root_commit="$(resolve_root_commit "${release_branch}" "${main_branch}")"

  [[ "${resolved_commit}" == "${root_commit}" ]]
}

cmd_is_hotfix_commit() {
  local commit="${1}"
  local release_branch="${2}"
  local main_branch="${3:-main}"
  local resolved_commit root_commit release_commit

  resolved_commit="$(resolve_commit "${commit}")"
  root_commit="$(resolve_root_commit "${release_branch}" "${main_branch}")"
  release_commit="$(resolve_commit "${release_branch}")"

  git merge-base --is-ancestor "${resolved_commit}" "${release_commit}" &&
    ! git merge-base --is-ancestor "${resolved_commit}" "${root_commit}"
}

cmd_is_hotfix_merge() {
  local commit="${1}"
  local main_branch="${2:-origin/main}"
  local resolved_commit release_branch parent
  local -a candidates=()

  resolved_commit="$(resolve_commit "${commit}")"
  mapfile -t candidates < <(git for-each-ref --format='%(refname:short)' 'refs/remotes/origin/release/*' 'refs/heads/release/*')

  for release_branch in "${candidates[@]}"; do
    if cmd_is_hotfix_commit "${resolved_commit}" "${release_branch}" "${main_branch}"; then
      return 0
    fi

    while read -r parent; do
      if [[ -n "${parent}" ]] && cmd_is_hotfix_commit "${parent}" "${release_branch}" "${main_branch}"; then
        return 0
      fi
    done < <(git rev-list --parents -n 1 "${resolved_commit}" | cut -d' ' -f3- | tr ' ' '\n')
  done

  return 1
}

main() {
  case "${1:-}" in
    is-hotfix-commit)
      require_arg "${2:-}" "commit"
      require_arg "${3:-}" "release_branch"
      validate_branch_name "${3#origin/}" '^release/'
      cmd_is_hotfix_commit "${2}" "${3}" "${4:-main}"
      ;;
    is-hotfix-merge)
      require_arg "${2:-}" "commit"
      cmd_is_hotfix_merge "${2}" "${3:-origin/main}"
      ;;
    is-root-commit)
      require_arg "${2:-}" "commit"
      require_arg "${3:-}" "release_branch"
      validate_branch_name "${3}" '^release/'
      cmd_is_root_commit "${2}" "${3}" "${4:-main}"
      ;;
    get-commits)
      require_arg "${2:-}" "release_branch"
      validate_branch_name "${2}" '^release/'
      cmd_get_release_branch_commits "${2}" "${3:-main}"
      ;;
    *)
      echo "Usage: $0 {get-commits RELEASE_BRANCH [MAIN_BRANCH]|is-root-commit COMMIT RELEASE_BRANCH [MAIN_BRANCH]|is-hotfix-commit COMMIT RELEASE_BRANCH [MAIN_BRANCH]|is-hotfix-merge COMMIT [MAIN_BRANCH]}" >&2
      exit 1
      ;;
  esac
}

main "$@"

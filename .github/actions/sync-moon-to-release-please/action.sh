#!/usr/bin/env bash
# Synchronizes publishable Moon projects into Release Please configuration.
set -euo pipefail

require_tools() {
  local tool
  for tool in moon jq diff; do
    command -v "${tool}" >/dev/null || { echo "${tool} not found" >&2; exit 1; }
  done
}

discover_release_files() {
  shopt -s nullglob
  configs=(release-please-config--*.json)
  shopt -u nullglob
  ((${#configs[@]} > 0)) || { echo "No release-please-config--*.json files found." >&2; exit 1; }

  manifest_file=".release-please-manifest.json"
  [[ -f "${manifest_file}" ]] || { echo "${manifest_file} not found." >&2; exit 1; }
}


moon_query_to_project_map() {
  local rows
  rows="$(moon query projects | jq -r '
    .projects[]?
    | select((.source | strings) != "")
    | select((.tasks // {}) | has("publish"))
    | [.id, .source, (.layer // .config.layer // "unknown")]
    | @tsv
  ')"

  while IFS=$'\t' read -r id source layer; do
    [[ -n "${id}" ]] || continue
    local release_type version

    # Version-source boundary. Add future package formats here; never guess.
    if [[ -f "${source}/package.json" ]]; then
      release_type="node"
      version="$(jq -er '.version | select(type == "string" and length > 0)' "${source}/package.json")" || {
        echo "Missing package version: ${source}/package.json" >&2
        return 1
      }
    else
      echo "Unsupported version source for publishable Moon project '${id}' at '${source}'." >&2
      return 1
    fi

    [[ "${version}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || {
      echo "Malformed semantic version '${version}' for publishable Moon project '${id}'." >&2
      return 1
    }

    jq -nc \
      --arg source "${source}" \
      --arg component "${id}" \
      --arg group "${layer}" \
      --arg release_type "${release_type}" \
      --arg version "${version}" \
      '{key: $source, value: {component: $component, group: $group, "release-type": $release_type, version: $version}}'
  done <<<"${rows}" | jq -sc '{packages: (sort_by(.key) | from_entries)}'
}

project_map_to_config_packages() {
  jq -c '(.packages // {}) | with_entries(del(.value.version)) | to_entries | sort_by(.key) | from_entries' <<<"${1}"
}

project_map_to_manifest() {
  local project_map_json="${1}"
  local existing_manifest_json="${2:-}"
  [[ -n "${existing_manifest_json}" ]] || existing_manifest_json='{}'
  jq -c --argjson existing "${existing_manifest_json}" '
    reduce ((.packages // {}) | to_entries | sort_by(.key))[] as $package ({};
      .[$package.key] = ($existing[$package.key] // $package.value.version)
    )
  ' <<<"${project_map_json}"
}

json_equal() {
  diff -u <(jq -S . <<<"${1}") <(jq -S . <<<"${2}") >/dev/null
}

run_check() {
  discover_release_files

  local project_map expected_packages existing_manifest expected_manifest failed=0
  project_map="$(moon_query_to_project_map)"
  expected_packages="$(project_map_to_config_packages "${project_map}")"
  existing_manifest="$(jq -c . "${manifest_file}")"
  expected_manifest="$(project_map_to_manifest "${project_map}" "${existing_manifest}")"

  if ! json_equal "$(jq -c 'keys | sort' <<<"${existing_manifest}")" "$(jq -c 'keys | sort' <<<"${expected_manifest}")"; then
    echo "Manifest package keys differ from publishable Moon projects." >&2
    failed=1
  fi

  local config actual_packages
  for config in "${configs[@]}"; do
    actual_packages="$(jq -c '.packages // {}' "${config}")"
    if ! json_equal "${actual_packages}" "${expected_packages}"; then
      echo "Release Please package metadata differs in ${config}." >&2
      diff -u <(jq -S . <<<"${actual_packages}") <(jq -S . <<<"${expected_packages}") >&2 || true
      failed=1
    fi
  done

  ((failed == 0)) || exit 1
  echo "release-please manifest and configs match publishable Moon projects."
}

run_sync() {
  discover_release_files

  local project_map expected_packages existing_manifest expected_manifest tmp config
  project_map="$(moon_query_to_project_map)"
  expected_packages="$(project_map_to_config_packages "${project_map}")"
  existing_manifest="$(jq -c . "${manifest_file}")"
  expected_manifest="$(project_map_to_manifest "${project_map}" "${existing_manifest}")"

  tmp="$(mktemp)"
  jq -S . <<<"${expected_manifest}" >"${tmp}"
  mv "${tmp}" "${manifest_file}"

  for config in "${configs[@]}"; do
    tmp="$(mktemp)"
    jq --argjson packages "${expected_packages}" '.packages = $packages' "${config}" >"${tmp}"
    mv "${tmp}" "${config}"
  done
}

main() {
  require_tools
  case "${1:-sync}" in
    check) run_check ;;
    sync) run_sync ;;
    *) echo "Usage: $0 [check|sync]" >&2; exit 2 ;;
  esac
}

main "$@"

#!/usr/bin/env bash
# Keeps release-please package keys aligned with Moon project discovery.
#
# Moon is the source of truth for project membership. release-please still
# needs static JSON files, so this script derives the package list from
# `moon query projects` and rewrites/checks:
#   - .release-please-manifest.json
#   - release-please-config--*.json
#
# Only Moon projects with a `publish` task are included. A package.json makes
# the project a node release; a Cargo.toml makes it a rust release.
# Existing manifest versions are preserved so sync does not accidentally bump
# or reset release history.
set -euo pipefail

require_tools() {
  command -v moon >/dev/null || {
    echo "moon not found" >&2
    exit 1
  }
  command -v jq >/dev/null || {
    echo "jq not found" >&2
    exit 1
  }
  command -v diff >/dev/null || {
    echo "diff not found" >&2
    exit 1
  }
}

discover_release_files() {
  # The branch-mode configs are kept in lockstep. The base
  # release-please-config.json is intentionally not matched here.
  shopt -s nullglob
  configs=(release-please-config--*.json)
  shopt -u nullglob

  if [[ ${#configs[@]} -eq 0 ]]; then
    echo "No release-please-config--*.json files found." >&2
    exit 1
  fi

  manifest_file=".release-please-manifest.json"
  if [[ ! -f "${manifest_file}" ]]; then
    echo "${manifest_file} not found." >&2
    exit 1
  fi
}

# Builds the normalized package map used by the two release-please formats.
#
# Output shape:
#   { "packages": { "path/to/package": { "group": "...", "release-type": "node|rust", "version": "..." } } }
#
# The temporary `version` field is only for manifest generation. Config files
# do not carry versions.
moon_query_to_project_map() {
  moon query projects | jq -r '
    .projects[]?
    | select((.source | strings) != "")
    | select((.tasks // {}) | has("publish"))
    | [.source, (.layer // .config.layer // "unknown")]
    | @tsv
  ' | while IFS=$'\t' read -r source layer; do
    local package_file="${source}/package.json"
    local cargo_file="${source}/Cargo.toml"
    local release_type version

    # The publish task is the inclusion marker. The package file only decides
    # which release-please strategy and starting version to use.
    if [[ -f "${package_file}" ]]; then
      release_type="node"
      version="$(jq -r '.version // "0.1.0"' "${package_file}")"
    elif [[ -f "${cargo_file}" ]]; then
      release_type="rust"
      version="$(awk '
        /^\[package\]$/ { in_package = 1; next }
        /^\[/ { in_package = 0 }
        in_package && $1 == "version" {
          gsub(/"/, "", $3)
          print $3
          exit
        }
      ' "${cargo_file}")"
      version="${version:-0.1.0}"
    else
      echo "Skipping ${source}: publish task exists but no package.json or Cargo.toml was found." >&2
      continue
    fi

    local group="${layer}"
    if [[ "${source}" == pkgs/provider-* ]]; then
      group="provider"
    fi

    jq -nc --arg source "${source}" --arg group "${group}" --arg release_type "${release_type}" --arg version "${version}" '{
      key: $source,
      value: {
        group: $group,
        "release-type": $release_type,
        version: $version
      }
    }'
  done | jq -sc '{ packages: (sort_by(.key) | from_entries) }'
}

# Converts the normalized package map into release-please config `.packages`.
# Config packages describe release behavior, not current versions.
moon_query_to_rp_config_packages() {
  local project_map_json="${1}"
  jq -c '
    (.packages // {})
    | with_entries(del(.value.version))
    | to_entries
    | sort_by(.key)
    | from_entries
  ' <<<"${project_map_json}"
}

# Converts the normalized package map into .release-please-manifest.json.
# Existing versions win. New packages fall back to package.json version, then
# 0.1.0 as a last resort.
moon_query_to_rp_manifest() {
  local project_map_json="${1}"
  local existing_manifest_json="${2:-}"
  if [[ -z "${existing_manifest_json}" ]]; then
    existing_manifest_json='{}'
  fi

  jq -c --argjson existing "${existing_manifest_json}" '
    (.packages // {} | to_entries | sort_by(.key)) as $packages
    | reduce $packages[] as $package ({};
        .[$package.key] = ($existing[$package.key] // $package.value.version // "0.1.0")
      )
  ' <<<"${project_map_json}"
}

print_key_diff() {
  # This intentionally compares only keys. release-please owns manifest
  # version values, and this sync action owns membership.
  local label="${1}"
  local expected_json="${2}"
  local actual_json="${3}"

  local expected_file actual_file
  expected_file="$(mktemp)"
  actual_file="$(mktemp)"

  jq -r 'keys[]' <<<"${expected_json}" >"${expected_file}"
  jq -r 'keys[]' <<<"${actual_json}" >"${actual_file}"

  if ! diff -u "${actual_file}" "${expected_file}" >/dev/null; then
    echo "${label} key mismatch" >&2
    diff -u "${actual_file}" "${expected_file}" >&2 || true
    rm -f "${expected_file}" "${actual_file}"
    return 1
  fi

  rm -f "${expected_file}" "${actual_file}"
}

run_check() {
  # CI mode: fail when package membership drifted, but do not rewrite files.
  discover_release_files

  local project_map expected_packages existing_manifest expected_manifest
  project_map="$(moon_query_to_project_map)"
  expected_packages="$(moon_query_to_rp_config_packages "${project_map}")"
  existing_manifest="$(jq -c '.' "${manifest_file}")"
  expected_manifest="$(moon_query_to_rp_manifest "${project_map}" "${existing_manifest}")"

  local failed=0

  if ! print_key_diff \
    "Manifest (.release-please-manifest.json)" \
    "${expected_manifest}" \
    "${existing_manifest}"; then
    failed=1
  fi

  local config actual_packages
  for config in "${configs[@]}"; do
    actual_packages="$(jq -c '.packages // {}' "${config}")"
    if ! print_key_diff "Config (${config}) .packages" "${expected_packages}" "${actual_packages}"; then
      failed=1
    fi
  done

  if [[ "${failed}" -ne 0 ]]; then
    exit 1
  fi

  echo "release-please manifest/config keys are in sync with moon query output."
}

run_sync() {
  # Maintainer mode: rewrite release-please files from current Moon state.
  discover_release_files

  local project_map expected_packages existing_manifest expected_manifest
  project_map="$(moon_query_to_project_map)"
  expected_packages="$(moon_query_to_rp_config_packages "${project_map}")"
  existing_manifest="$(jq -c '.' "${manifest_file}")"
  expected_manifest="$(moon_query_to_rp_manifest "${project_map}" "${existing_manifest}")"

  local tmp config

  echo "Syncing ${manifest_file}"
  tmp="$(mktemp)"
  jq '.' <<<"${expected_manifest}" >"${tmp}"
  mv "${tmp}" "${manifest_file}"

  for config in "${configs[@]}"; do
    echo "Syncing ${config}"
    tmp="$(mktemp)"
    jq --argjson packages "${expected_packages}" '
      .packages = $packages
      | .packages |= (to_entries | sort_by(.key) | from_entries)
    ' "${config}" >"${tmp}"
    mv "${tmp}" "${config}"
  done
}

usage() {
  cat <<'EOF'
Usage:
  action.sh check   # compare moon-derived keys vs release-please files
  action.sh sync    # rewrite release-please files from moon-derived data

Default command: sync
EOF
}

main() {
  require_tools

  local cmd="${1:-sync}"
  case "${cmd}" in
  check)
    run_check
    ;;
  sync)
    run_sync
    ;;
  -h | --help | help)
    usage
    ;;
  *)
    echo "Unknown command: ${cmd}" >&2
    usage >&2
    exit 2
    ;;
  esac
}

main "$@"

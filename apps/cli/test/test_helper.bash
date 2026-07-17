setup_agentboard_test() {
  export TMP="$BATS_TEST_TMPDIR"
  export XDG_CONFIG_HOME="$TMP/config"
  export XDG_DATA_HOME="$TMP/data"
  export AB="$BATS_TEST_DIRNAME/../../../target/debug/agentboard"
  export ORIGINAL_PATH="$PATH"
  export QMD_ITEMS="$TMP/items"
  export WATCH_PID=""
  mkdir -p "$TMP/bin" "$QMD_ITEMS"
  export PATH="$TMP/bin:$PATH"
  install_qmd_stub
}

teardown_agentboard_test() {
  if [ -n "${WATCH_PID:-}" ] && kill -0 "$WATCH_PID" 2>/dev/null; then
    kill -INT "$WATCH_PID" 2>/dev/null || true
    wait "$WATCH_PID" 2>/dev/null || true
  fi
}

install_qmd_stub() {
  cat > "$TMP/bin/qmd" <<'EOF'
#!/bin/bash
set -euo pipefail

if [ -n "${QMD_LOG:-}" ]; then
  printf '%s\n' "$*" >> "$QMD_LOG"
fi

case "${1:-}" in
  query)
    if [ "${QMD_QUERY_EXIT:-0}" != 0 ]; then
      echo "qmd query fixture failure" >&2
      exit "$QMD_QUERY_EXIT"
    fi
    if [ -n "${QMD_QUERY_SLEEP:-}" ]; then
      /bin/sleep "$QMD_QUERY_SLEEP"
    fi
    collection=""
    previous=""
    for argument in "$@"; do
      if [ "$previous" = "-c" ]; then
        collection="$argument"
        break
      fi
      previous="$argument"
    done
    root="$QMD_ITEMS"
    if [ -n "$collection" ] && [ -d "$QMD_ITEMS/$collection" ]; then
      root="$QMD_ITEMS/$collection"
    fi
    /usr/bin/python3 - "$root" <<'PY'
import json, pathlib, sys
root = pathlib.Path(sys.argv[1])
print(json.dumps([{"path": str(path)} for path in sorted(root.glob("*.md"))]))
PY
    ;;
  get)
    if [ "${QMD_GET_EXIT:-0}" != 0 ]; then
      echo "qmd get fixture failure" >&2
      exit "$QMD_GET_EXIT"
    fi
    /bin/cat "$2"
    ;;
  --version)
    echo qmd-test
    ;;
  *)
    exit 2
    ;;
esac
EOF
  chmod +x "$TMP/bin/qmd"
}

write_item_at() {
  local root="$1"
  local id="$2"
  local title="${3:-Item $id}"
  local status="${4:-ready}"
  mkdir -p "$root"
  cat > "$root/$id.md" <<EOF
---
id: "$id"
title: "$title"
status: "$status"
---
Body for $id
EOF
}

write_item() {
  write_item_at "$QMD_ITEMS" "$@"
}

write_collection_item() {
  local collection="$1"
  shift
  write_item_at "$QMD_ITEMS/$collection" "$@"
}

write_workspace() {
  local cmd="$1"
  cat > "$TMP/workspace.toml" <<EOF
[[sources]]
id = "md"

[sources.source]
kind = "qmd"
collections = ["test"]
query = "status:ready"

[[sources.actions]]
uses = "agentboard/run-cmd"
[sources.actions.with]
cmd = '''$cmd'''
EOF
}

store_files() {
  find "$XDG_DATA_HOME/agentboard" -type f 2>/dev/null | sort
}

items_store_file() {
  find "$XDG_DATA_HOME/agentboard" -type f -name 'items-*.jsonl' | head -n 1
}

actions_store_file() {
  find "$XDG_DATA_HOME/agentboard" -type f -name 'actions-*.jsonl' | head -n 1
}

wait_for_pattern() {
  local file="$1"
  local pattern="$2"
  local attempts="${3:-100}"
  local index
  for ((index = 0; index < attempts; index++)); do
    if grep -q "$pattern" "$file" 2>/dev/null; then
      return 0
    fi
    sleep 0.05
  done
  echo "pattern '$pattern' not found in $file" >&2
  return 1
}
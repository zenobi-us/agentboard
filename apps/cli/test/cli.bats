#!/usr/bin/env bats

setup() {
  export TMP="$BATS_TEST_TMPDIR"
  export XDG_CONFIG_HOME="$TMP/config"
  export XDG_DATA_HOME="$TMP/data"
  export AB="$BATS_TEST_DIRNAME/../../../target/debug/agentboard"
  mkdir -p "$TMP/items" "$TMP/bin"
  export PATH="$TMP/bin:$PATH"
  cat > "$TMP/bin/qmd" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
case "$1" in
  query)
    python - "$QMD_ITEMS" <<'PY'
import json, pathlib, sys
root = pathlib.Path(sys.argv[1])
print(json.dumps([{"path": str(p)} for p in sorted(root.glob("*.md"))]))
PY
    ;;
  get)
    cat "$2"
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
  export QMD_ITEMS="$TMP/items"
}

write_item() {
  local id="$1"
  cat > "$TMP/items/$id.md" <<EOF
---
id: $id
title: Item $id
status: ready
---
Body
EOF
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

@test "workspaces lists named configs in order" {
  mkdir -p "$XDG_CONFIG_HOME/agentboard"
  touch "$XDG_CONFIG_HOME/agentboard/zeta.toml"
  touch "$XDG_CONFIG_HOME/agentboard/alpha.toml"
  touch "$XDG_CONFIG_HOME/agentboard/ignored.txt"

  run "$AB" workspaces

  [ "$status" -eq 0 ]
  [ "$output" = $'alpha\nzeta' ]
}

@test "run success" {
  write_item AB-1
  write_workspace "echo \$AGENTBOARD_ITEM_ID >> '$TMP/ran'"

  run "$AB" run "$TMP/workspace.toml"

  [ "$status" -eq 0 ]
  [ "$(cat "$TMP/ran")" = "AB-1" ]
  store_files | grep '/items.jsonl$'
  store_files | grep '/actions.jsonl$'
}

@test "no rerun after success" {
  write_item AB-1
  write_workspace "echo run >> '$TMP/ran'"

  run "$AB" run "$TMP/workspace.toml"
  [ "$status" -eq 0 ]
  run "$AB" run "$TMP/workspace.toml"
  [ "$status" -eq 0 ]

  [ "$(wc -l < "$TMP/ran")" -eq 1 ]
}

@test "retry after failure" {
  write_item AB-1
  write_workspace "if [ ! -f '$TMP/marker' ]; then touch '$TMP/marker'; exit 1; fi; echo ok >> '$TMP/ran'"

  run "$AB" run "$TMP/workspace.toml"
  [ "$status" -ne 0 ]
  run "$AB" run "$TMP/workspace.toml"
  [ "$status" -eq 0 ]

  [ "$(cat "$TMP/ran")" = "ok" ]
}

@test "duplicate IDs fail" {
  write_item AB-1
  cp "$TMP/items/AB-1.md" "$TMP/items/dupe.md"
  write_workspace "true"

  run "$AB" run "$TMP/workspace.toml"

  [ "$status" -ne 0 ]
  [[ "$output" == *"duplicate item id AB-1"* ]]
}

@test "dry-run writes nothing" {
  write_item AB-1
  write_workspace "echo bad > '$TMP/ran'"

  run "$AB" run "$TMP/workspace.toml" --dry-run

  [ "$status" -eq 0 ]
  [ ! -e "$TMP/ran" ]
  [ -z "$(store_files)" ]
}

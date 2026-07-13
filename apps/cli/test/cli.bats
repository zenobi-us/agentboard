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

@test "workspace list and compatibility alias list named configs in order" {
  mkdir -p "$XDG_CONFIG_HOME/agentboard"
  touch "$XDG_CONFIG_HOME/agentboard/zeta.toml"
  touch "$XDG_CONFIG_HOME/agentboard/alpha.toml"
  touch "$XDG_CONFIG_HOME/agentboard/ignored.txt"

  run "$AB" workspace list
  [ "$status" -eq 0 ]
  [ "$output" = $'alpha\nzeta' ]

  run "$AB" workspaces
  [ "$status" -eq 0 ]
  [ "$output" = $'alpha\nzeta' ]
}

@test "workspace init creates empty named workspace and refuses overwrite" {
  run "$AB" workspace init work

  [ "$status" -eq 0 ]
  [ "$output" = "$XDG_CONFIG_HOME/agentboard/work.toml" ]
  [ "$(cat "$XDG_CONFIG_HOME/agentboard/work.toml")" = "sources = []" ]

  run "$AB" workspace init work
  [ "$status" -ne 0 ]
  [[ "$output" == *"already exists"* ]]
  [[ "$output" == *"$XDG_CONFIG_HOME/agentboard/work.toml"* ]]
}

@test "run success" {
  write_item AB-1
  write_workspace "echo \$AGENTBOARD_ITEM_ID >> '$TMP/ran'"

  run "$AB" run "$TMP/workspace.toml"

  [ "$status" -eq 0 ]
  [ "$(cat "$TMP/ran")" = "AB-1" ]
  store_files | grep '/items-.*\.jsonl$'
  store_files | grep '/actions-.*\.jsonl$'
}

@test "run writes progress to stderr and metadata-only JSONL" {
  write_item AB-1
  write_workspace "echo action-output"

  "$AB" --color never --log-file "$TMP/events.jsonl" run "$TMP/workspace.toml" >"$TMP/stdout" 2>"$TMP/stderr"

  [ ! -s "$TMP/stdout" ]
  grep -q "run .* starting" "$TMP/stderr"
  grep -q "source md complete: 1 items" "$TMP/stderr"
  grep -q '"stage":"run.complete"' "$TMP/events.jsonl"
  ! grep -q 'action-output' "$TMP/events.jsonl"
  ! grep -q '"stdout"' "$TMP/events.jsonl"
  ! grep -q $'\033' "$TMP/stderr"
}

@test "verbose shows successful action output" {
  write_item AB-1
  write_workspace "echo visible-out; echo visible-err >&2"

  run "$AB" -v --color never run "$TMP/workspace.toml"

  [ "$status" -eq 0 ]
  [[ "$output" == *"visible-out"* ]]
  [[ "$output" == *"visible-err"* ]]
}

@test "watch reports successful cycle outcome" {
  write_item AB-1
  write_workspace "true"

  "$AB" --color never --log-file "$TMP/watch.jsonl" watch "$TMP/workspace.toml" --interval 60s >"$TMP/stdout" 2>"$TMP/stderr" &
  local pid=$!
  for _ in {1..50}; do
    grep -q 'watch.cycle.complete' "$TMP/watch.jsonl" 2>/dev/null && break
    sleep 0.05
  done
  kill -INT "$pid"
  wait "$pid"

  grep -q 'watch .* cycle 1 complete' "$TMP/stderr"
  grep -q '"stage":"watch.cycle.complete"' "$TMP/watch.jsonl"
  grep -q '"outcome":"pass"' "$TMP/watch.jsonl"
  grep -q '"run":' "$TMP/watch.jsonl"
}

@test "quiet suppresses successful progress" {
  write_item AB-1
  write_workspace "true"

  "$AB" -q run "$TMP/workspace.toml" >"$TMP/stdout" 2>"$TMP/stderr"

  [ ! -s "$TMP/stdout" ]
  [ ! -s "$TMP/stderr" ]
}

@test "failed action output is shown without verbose mode" {
  write_item AB-1
  write_workspace "echo visible-out; echo visible-err >&2; exit 1"

  run "$AB" --color never run "$TMP/workspace.toml"

  [ "$status" -ne 0 ]
  [[ "$output" == *"visible-out"* ]]
  [[ "$output" == *"visible-err"* ]]
}

@test "doctor distinguishes fetched count from unknown availability" {
  write_item AB-1
  write_workspace "true"

  run "$AB" --color never doctor "$TMP/workspace.toml"

  [ "$status" -eq 0 ]
  [[ "$output" == *"source md reachable (1 fetched; limit 50; available unknown)"* ]]
  [[ "$output" == *"actions [1]"* ]]
  [[ "$output" == *"- agentboard/run-cmd [ok]"* ]]
}

@test "doctor reports all independent source failures" {
  cat > "$TMP/workspace.toml" <<'EOF'
[[sources]]
id = "one"
[sources.source]
kind = "qmd"
collections = ["test"]
query = "status:ready"

[[sources]]
id = "two"
[sources.source]
kind = "qmd"
collections = ["test"]
query = "status:ready"
EOF
  cat > "$TMP/bin/qmd" <<'EOF'
#!/usr/bin/env bash
if [ "$1" = "--version" ]; then exit 0; fi
exit 9
EOF
  chmod +x "$TMP/bin/qmd"

  run "$AB" --color never doctor "$TMP/workspace.toml"

  [ "$status" -ne 0 ]
  [[ "$output" == *"fail source one"* ]]
  [[ "$output" == *"fail source two"* ]]
  [[ "$output" == *"2 check(s)"* ]]
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

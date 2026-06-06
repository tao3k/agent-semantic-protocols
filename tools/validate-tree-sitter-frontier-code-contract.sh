#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

asp() {
  if [[ -n "${SEMANTIC_AGENT_PROTOCOL_BIN:-}" ]]; then
    "$SEMANTIC_AGENT_PROTOCOL_BIN" "$@"
  elif command -v cargo >/dev/null 2>&1; then
    cargo run -q -p agent-semantic-protocol --bin asp -- "$@"
  elif [[ -x "$repo_root/target/debug/asp" ]]; then
    "$repo_root/target/debug/asp" "$@"
  else
    printf 'missing cargo and SEMANTIC_AGENT_PROTOCOL_BIN for asp contract gate\n' >&2
    exit 127
  fi
}

assert_contains() {
  local value="$1"
  local needle="$2"
  local label="$3"
  if [[ "$value" != *"$needle"* ]]; then
    printf '%s: expected output to contain %q\n' "$label" "$needle" >&2
    printf '%s\n' "$value" >&2
    exit 1
  fi
}

assert_not_contains() {
  local value="$1"
  local needle="$2"
  local label="$3"
  if [[ "$value" == *"$needle"* ]]; then
    printf '%s: expected output to omit %q\n' "$label" "$needle" >&2
    printf '%s\n' "$value" >&2
    exit 1
  fi
}

assert_json_string() {
  local value="$1"
  local key="$2"
  local expected="$3"
  local label="$4"
  if [[ "$value" != *"\"$key\":\"$expected\""* && "$value" != *"\"$key\": \"$expected\""* ]]; then
    printf '%s: expected JSON field %s=%q\n' "$label" "$key" "$expected" >&2
    printf '%s\n' "$value" >&2
    exit 1
  fi
}

assert_json_false() {
  local value="$1"
  local key="$2"
  local label="$3"
  if [[ "$value" != *"\"$key\":false"* && "$value" != *"\"$key\": false"* ]]; then
    printf '%s: expected JSON field %s=false\n' "$label" "$key" >&2
    printf '%s\n' "$value" >&2
    exit 1
  fi
}

assert_locate_has_no_cache_noise() {
  local value="$1"
  local label="$2"
  assert_not_contains "$value" "artifactId" "$label"
  assert_not_contains "$value" "sqlite" "$label"
  assert_not_contains "$value" "cacheRoot" "$label"
  assert_not_contains "$value" "receipt" "$label"
}

assert_code_has_no_frontier_noise() {
  local value="$1"
  local label="$2"
  assert_not_contains "$value" "[query-treesitter]" "$label"
  assert_not_contains "$value" "frontier=" "$label"
  assert_not_contains "$value" "artifactId" "$label"
  assert_not_contains "$value" "sqlite" "$label"
  assert_not_contains "$value" "cacheRoot" "$label"
}

rust_locate="$(
  asp rust query \
    --treesitter-query '(function_item name: (identifier) @function.name)' \
    --selector src/cli/query.rs \
    languages/rust-lang-project-harness
)"
assert_contains "$rust_locate" "[query-treesitter]" "rust locate"
assert_contains "$rust_locate" "frontier=I.code" "rust locate"
assert_contains "$rust_locate" "omit=code,full-node-list,capture-text" "rust locate"
assert_contains "$rust_locate" "ts=identifier/name" "rust locate"
assert_contains "$rust_locate" "parse_query" "rust locate"
assert_not_contains "$rust_locate" "pub(super) fn parse_query" "rust locate"
assert_locate_has_no_cache_noise "$rust_locate" "rust locate"

rust_code="$(
  asp rust query \
    --treesitter-query '(function_item name: (identifier) @function.name (#eq? @function.name "parse_query"))' \
    --selector src/cli/query.rs \
    --code \
    languages/rust-lang-project-harness
)"
assert_contains "$rust_code" "pub(super) fn parse_query" "rust code"
assert_not_contains "$rust_code" "[query-treesitter]" "rust code"
assert_code_has_no_frontier_noise "$rust_code" "rust code"

rust_json="$(
  asp rust query \
    --treesitter-query '(function_item name: (identifier) @function.name (#eq? @function.name "parse_query"))' \
    --selector src/cli/query.rs \
    --json \
    languages/rust-lang-project-harness
)"
assert_json_string "$rust_json" "schemaId" "agent.semantic-protocols.semantic-tree-sitter-query" "rust json"
assert_json_string "$rust_json" "adapterMode" "native-projection" "rust json"
assert_json_string "$rust_json" "compatibilityLevel" "native-only" "rust json"
assert_contains "$rust_json" '"nativeFactRefs": [' "rust json"
assert_contains "$rust_json" 'rust:item:src/cli/query.rs:' "rust json"
assert_contains "$rust_json" ':parse_query' "rust json"
assert_json_false "$rust_json" "rawSourceStored" "rust json"

ts_locate="$(
  asp typescript query \
    --treesitter-query '(function_declaration name: (identifier) @function.name)' \
    --selector src/cli/protocol-tree-sitter-query.ts \
    languages/typescript-lang-project-harness
)"
assert_contains "$ts_locate" "src/cli/protocol-tree-sitter-query.ts:53" "typescript locate"
assert_contains "$ts_locate" "parseTreeSitterQueryArgs" "typescript locate"
assert_not_contains "$ts_locate" "export function parseTreeSitterQueryArgs" "typescript locate"
assert_locate_has_no_cache_noise "$ts_locate" "typescript locate"

ts_code="$(
  asp typescript query \
    --treesitter-query '(function_declaration name: (identifier) @function.name (#eq? @function.name "parseTreeSitterQueryArgs"))' \
    --selector src/cli/protocol-tree-sitter-query.ts \
    --code \
    languages/typescript-lang-project-harness
)"
assert_contains "$ts_code" "export function parseTreeSitterQueryArgs" "typescript code"
assert_code_has_no_frontier_noise "$ts_code" "typescript code"

ts_json="$(
  asp typescript query \
    --treesitter-query '(function_declaration name: (identifier) @function.name (#eq? @function.name "parseTreeSitterQueryArgs"))' \
    --selector src/cli/protocol-tree-sitter-query.ts \
    --json \
    languages/typescript-lang-project-harness
)"
assert_json_string "$ts_json" "schemaId" "agent.semantic-protocols.semantic-tree-sitter-query" "typescript json"
assert_json_string "$ts_json" "adapterMode" "native-projection" "typescript json"
assert_json_string "$ts_json" "compatibilityLevel" "native-only" "typescript json"
assert_contains "$ts_json" '"nativeFactRefs"' "typescript json"
assert_contains "$ts_json" 'typescript:item:src/cli/protocol-tree-sitter-query.ts:53:58:parseTreeSitterQueryArgs' "typescript json"
assert_json_false "$ts_json" "rawSourceStored" "typescript json"
assert_json_string "$ts_json" "nodeType" "identifier" "typescript json"
assert_json_string "$ts_json" "field" "name" "typescript json"
assert_json_string "$ts_json" "nativeNodeType" "function_declaration" "typescript json"

python_locate="$(
  asp python query \
    --treesitter-query '(function_definition name: (identifier) @function.name)' \
    --selector src/python_lang_project_harness/_cli_query.py \
    languages/python-lang-project-harness
)"
assert_contains "$python_locate" "src/python_lang_project_harness/_cli_query.py:20" "python locate"
assert_contains "$python_locate" "run_query_command" "python locate"
assert_not_contains "$python_locate" "def run_query_command" "python locate"
assert_locate_has_no_cache_noise "$python_locate" "python locate"

python_code="$(
  asp python query \
    --treesitter-query '(function_definition name: (identifier) @function.name (#eq? @function.name "run_query_command"))' \
    --selector src/python_lang_project_harness/_cli_query.py \
    --code \
    languages/python-lang-project-harness
)"
assert_contains "$python_code" "def run_query_command" "python code"
assert_code_has_no_frontier_noise "$python_code" "python code"

python_json="$(
  asp python query \
    --treesitter-query '(function_definition name: (identifier) @function.name (#eq? @function.name "run_query_command"))' \
    --selector src/python_lang_project_harness/_cli_query.py \
    --json \
    languages/python-lang-project-harness
)"
assert_json_string "$python_json" "schemaId" "agent.semantic-protocols.semantic-tree-sitter-query" "python json"
assert_json_string "$python_json" "adapterMode" "native-projection" "python json"
assert_json_string "$python_json" "compatibilityLevel" "native-only" "python json"
assert_contains "$python_json" '"nativeFactRefs"' "python json"
assert_contains "$python_json" 'python:ast:src/python_lang_project_harness/_cli_query.py:20:60:run_query_command' "python json"
assert_json_false "$python_json" "rawSourceStored" "python json"
assert_json_string "$python_json" "nodeType" "identifier" "python json"
assert_json_string "$python_json" "field" "name" "python json"
assert_json_string "$python_json" "nativeNodeType" "function_definition" "python json"

printf 'tree-sitter frontier/code contract is valid\n'

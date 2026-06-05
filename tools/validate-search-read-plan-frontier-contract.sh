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

assert_no_cache_noise() {
  local value="$1"
  local label="$2"
  assert_not_contains "$value" "artifactId" "$label"
  assert_not_contains "$value" "sqlite" "$label"
  assert_not_contains "$value" "cacheRoot" "$label"
  assert_not_contains "$value" "receipt" "$label"
}

assert_search_frontier() {
  local value="$1"
  local label="$2"
  assert_contains "$value" "[search-fzf]" "$label"
  assert_contains "$value" "legend:" "$label"
  assert_contains "$value" "frontier ID.next" "$label"
  assert_contains "$value" "frontier=" "$label"
  assert_contains "$value" "entries=owner-query" "$label"
  assert_no_cache_noise "$value" "$label"
}

rust_search="$(
  asp rust query \
    --from-hook direct-source-read \
    --selector '**/*.rs' \
    --term parse_query \
    --surface owners,tests \
    --view seeds \
    languages/rust-lang-project-harness
)"
assert_search_frontier "$rust_search" "rust search frontier"
assert_contains "$rust_search" "O=owner:path(" "rust search frontier"
assert_not_contains "$rust_search" "pub(super) fn parse_query" "rust search frontier"

typescript_search="$(
  asp typescript query \
    --from-hook direct-source-read \
    --selector '**/*.ts' \
    --term parseTreeSitterQueryArgs \
    --surface owners,tests \
    --view seeds \
    languages/typescript-lang-project-harness
)"
assert_search_frontier "$typescript_search" "typescript search frontier"
assert_contains "$typescript_search" "src/cli/protocol-tree-sitter-query.ts" "typescript search frontier"
assert_not_contains "$typescript_search" "export function parseTreeSitterQueryArgs" "typescript search frontier"

python_search="$(
  asp python query \
    --from-hook direct-source-read \
    --selector '**/*.py' \
    --term run_query_command \
    --surface owners,tests \
    --view seeds \
    languages/python-lang-project-harness
)"
assert_search_frontier "$python_search" "python search frontier"
assert_contains "$python_search" "src/python_lang_project_harness/_cli_query.py" "python search frontier"
assert_not_contains "$python_search" "def run_query_command" "python search frontier"

rust_read_plan="$(
  asp rust query \
    --from-hook direct-source-read \
    --selector src/cli/query.rs:1:260 \
    languages/rust-lang-project-harness
)"
assert_contains "$rust_read_plan" "[read-plan]" "rust read-plan"
assert_contains "$rust_read_plan" "mode=range-frontier" "rust read-plan"
assert_contains "$rust_read_plan" "frontier=S.code" "rust read-plan"
assert_contains "$rust_read_plan" "omit=code" "rust read-plan"
assert_contains "$rust_read_plan" "avoid=repeat-wide-read" "rust read-plan"
assert_contains "$rust_read_plan" "parse_query" "rust read-plan"
assert_not_contains "$rust_read_plan" "pub(super) fn parse_query" "rust read-plan"
assert_no_cache_noise "$rust_read_plan" "rust read-plan"

printf 'search/read-plan frontier contract is valid\n'

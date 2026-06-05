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

assert_pure_code() {
  local value="$1"
  local signature="$2"
  local label="$3"
  assert_contains "$value" "$signature" "$label"
  assert_not_contains "$value" "[read-owner]" "$label"
  assert_not_contains "$value" "[read-plan]" "$label"
  assert_not_contains "$value" "|code" "$label"
  assert_not_contains "$value" "text=" "$label"
  assert_not_contains "$value" "frontier=" "$label"
  assert_no_cache_noise "$value" "$label"
}

rust_read="$(
  asp rust query \
    --from-hook direct-source-read \
    --selector src/cli/query.rs:11:33 \
    languages/rust-lang-project-harness
)"
assert_contains "$rust_read" "[read-plan]" "rust exact read"
assert_contains "$rust_read" "mode=range-frontier" "rust exact read"
assert_contains "$rust_read" "frontier=S.code" "rust exact read"
assert_contains "$rust_read" "omit=code" "rust exact read"
assert_contains "$rust_read" "parse_query" "rust exact read"
assert_not_contains "$rust_read" "pub(super) fn parse_query" "rust exact read"
assert_no_cache_noise "$rust_read" "rust exact read"

rust_code="$(
  asp rust query \
    --from-hook direct-source-read \
    --selector src/cli/query.rs:11:33 \
    --code \
    languages/rust-lang-project-harness
)"
assert_pure_code "$rust_code" "pub(super) fn parse_query" "rust exact code"

typescript_read="$(
  asp typescript query \
    --from-hook direct-source-read \
    --selector src/cli/protocol-tree-sitter-query.ts:53:58 \
    languages/typescript-lang-project-harness
)"
assert_contains "$typescript_read" "[read-owner]" "typescript exact read"
assert_contains "$typescript_read" "window=1" "typescript exact read"
assert_contains "$typescript_read" "|read path=src/cli/protocol-tree-sitter-query.ts" "typescript exact read"
assert_contains "$typescript_read" "|code path=src/cli/protocol-tree-sitter-query.ts" "typescript exact read"
assert_contains "$typescript_read" "text=\"export function parseTreeSitterQueryArgs" "typescript exact read"
assert_no_cache_noise "$typescript_read" "typescript exact read"

typescript_code="$(
  asp typescript query \
    --from-hook direct-source-read \
    --selector src/cli/protocol-tree-sitter-query.ts:53:58 \
    --code \
    languages/typescript-lang-project-harness
)"
assert_pure_code "$typescript_code" "export function parseTreeSitterQueryArgs" "typescript exact code"

python_read="$(
  asp python query \
    --from-hook direct-source-read \
    --selector src/python_lang_project_harness/_cli_query.py:20:50 \
    languages/python-lang-project-harness
)"
assert_contains "$python_read" "[read-owner]" "python exact read"
assert_contains "$python_read" "window=1" "python exact read"
assert_contains "$python_read" "|read path=src/python_lang_project_harness/_cli_query.py" "python exact read"
assert_contains "$python_read" "|code path=src/python_lang_project_harness/_cli_query.py" "python exact read"
assert_contains "$python_read" "text=\"def run_query_command" "python exact read"
assert_no_cache_noise "$python_read" "python exact read"

python_code="$(
  asp python query \
    --from-hook direct-source-read \
    --selector src/python_lang_project_harness/_cli_query.py:20:50 \
    --code \
    languages/python-lang-project-harness
)"
assert_pure_code "$python_code" "def run_query_command" "python exact code"

printf 'exact direct-read contract is valid\n'

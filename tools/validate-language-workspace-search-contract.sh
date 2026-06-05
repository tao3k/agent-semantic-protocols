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

assert_asp_fails_contains() {
  local label="$1"
  local needle="$2"
  shift 2
  local value
  local status
  set +e
  value="$(asp "$@" 2>&1)"
  status=$?
  set -e
  if [[ "$status" -eq 0 ]]; then
    printf '%s: expected command to fail\n' "$label" >&2
    printf '%s\n' "$value" >&2
    exit 1
  fi
  assert_contains "$value" "$needle" "$label"
}

assert_workspace_graph() {
  local value="$1"
  local label="$2"
  assert_contains "$value" "[search-workspace]" "$label"
  assert_contains "$value" "legend:" "$label"
  assert_contains "$value" "frontier=" "$label"
  assert_not_contains "$value" "G>{}" "$label"
  assert_not_contains "$value" "frontier="'
' "$label"
  assert_not_contains "$value" "expected at most one PROJECT_ROOT argument" "$label"
}

assert_ingest_graph() {
  local value="$1"
  local label="$2"
  assert_contains "$value" "[search-ingest]" "$label"
  assert_not_contains "$value" "expected at most one PROJECT_ROOT argument" "$label"
}

rust_workspace="$(asp rust search workspace --view seeds languages/rust-lang-project-harness)"
assert_workspace_graph "$rust_workspace" "rust workspace"
assert_contains "$rust_workspace" "aliases: graph:{G=search,P=package}" "rust workspace"
assert_contains "$rust_workspace" "P=package:pkg(.)" "rust workspace"

rust_ingest="$(asp rust search ingest items tests --view seeds languages/rust-lang-project-harness)"
assert_ingest_graph "$rust_ingest" "rust ingest"

assert_asp_fails_contains \
  "rust ingest extra root" \
  "expected at most one PROJECT_ROOT argument" \
  rust search ingest items tests --view seeds languages/rust-lang-project-harness .

typescript_workspace="$(asp typescript search workspace --view seeds languages/typescript-lang-project-harness)"
assert_workspace_graph "$typescript_workspace" "typescript workspace"
assert_contains "$typescript_workspace" "O=owner:path(.)!owner" "typescript workspace"

typescript_ingest="$(asp typescript search ingest items tests --view seeds languages/typescript-lang-project-harness)"
assert_ingest_graph "$typescript_ingest" "typescript ingest"

assert_asp_fails_contains \
  "typescript ingest extra root" \
  "expected at most one PROJECT_ROOT argument" \
  typescript search ingest items tests --view seeds languages/typescript-lang-project-harness .

python_workspace="$(asp python search workspace --view seeds languages/python-lang-project-harness)"
assert_workspace_graph "$python_workspace" "python workspace"
assert_contains "$python_workspace" "O=owner:path(.)!owner" "python workspace"

python_ingest="$(asp python search ingest items tests --view seeds languages/python-lang-project-harness)"
assert_ingest_graph "$python_ingest" "python ingest"

assert_asp_fails_contains \
  "python ingest extra root" \
  "expected at most one PROJECT_ROOT argument" \
  python search ingest items tests --view seeds languages/python-lang-project-harness .

julia_workspace="$(asp julia search workspace --view seeds languages/JuliaLangProjectHarness.jl)"
assert_contains "$julia_workspace" "[search-workspace]" "julia workspace"
assert_not_contains "$julia_workspace" "expected at most one PROJECT_ROOT argument" "julia workspace"
if [[ "$julia_workspace" == *"legend:"* ]]; then
  assert_workspace_graph "$julia_workspace" "julia workspace"
  assert_contains "$julia_workspace" "aliases: graph:{G=search,O=owner}" "julia workspace"
else
  assert_contains "$julia_workspace" "scope=workspace" "julia workspace"
  assert_contains "$julia_workspace" "|seed owner:" "julia workspace"
fi

julia_ingest="$(asp julia search ingest owner tests --view seeds languages/JuliaLangProjectHarness.jl)"
assert_ingest_graph "$julia_ingest" "julia ingest"
if [[ "$julia_ingest" != *"legend:"* ]]; then
  assert_contains "$julia_ingest" "pipes=owner,tests" "julia ingest"
fi

assert_asp_fails_contains \
  "julia ingest extra root" \
  "expected at most one PROJECT_ROOT argument" \
  julia search ingest owner tests --view seeds languages/JuliaLangProjectHarness.jl .

rust_registry="$(asp rust agent doctor --json languages/rust-lang-project-harness)"
assert_contains "$rust_registry" '"method":"search/workspace"' "rust registry"
assert_contains "$rust_registry" '"acceptedPipes":["items","tests"]' "rust registry"

typescript_registry="$(asp typescript agent doctor --json languages/typescript-lang-project-harness)"
assert_contains "$typescript_registry" '"method":"search/workspace"' "typescript registry"
assert_contains "$typescript_registry" '"acceptedPipes":["items","tests"]' "typescript registry"

python_registry="$(asp python agent doctor --json languages/python-lang-project-harness)"
assert_contains "$python_registry" '"method":"search/workspace"' "python registry"
assert_contains "$python_registry" '"acceptedPipes":["items","tests"]' "python registry"

julia_registry="$(asp julia agent doctor --json languages/JuliaLangProjectHarness.jl)"
assert_contains "$julia_registry" '"method":"search/workspace"' "julia registry"
assert_contains "$julia_registry" '"acceptedPipes":["owner","tests"]' "julia registry"

printf 'language workspace/search ingest contract is valid\n'

#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

declare -a checks=(
  "languages/rust-lang-project-harness/Cargo.toml"
  "languages/typescript-lang-project-harness/package.json"
  "languages/python-lang-project-harness/pyproject.toml"
)

violations=0
for manifest in "${checks[@]}"; do
  path="$ROOT/$manifest"
  if [[ ! -f "$path" ]]; then
    continue
  fi
  if grep -En '(^|["[:space:]])(@?tree-sitter[^"[:space:]]*|tree_sitter[^"[:space:]]*)(["[:space:]]*:|[[:space:]]*=)' "$path"; then
    violations=$((violations + 1))
  fi
done

if (( violations > 0 )); then
  echo "language providers must not depend on tree-sitter runtime packages; ASP wrap owns tree-sitter query ABI/runtime" >&2
  exit 1
fi

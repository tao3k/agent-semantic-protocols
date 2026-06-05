#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

npm --prefix languages/typescript-lang-project-harness run build
cargo build -q -p agent-semantic-protocol --bin asp
cargo build -q --manifest-path languages/rust-lang-project-harness/Cargo.toml --features cli,search --bin rs-harness

shim_dir="$(mktemp -d)"
cleanup() {
  rm -rf "$shim_dir"
}
trap cleanup EXIT

cat >"$shim_dir/rs-harness" <<SH
#!/usr/bin/env bash
exec "$repo_root/languages/rust-lang-project-harness/target/debug/rs-harness" "\$@"
SH

cat >"$shim_dir/ts-harness" <<SH
#!/usr/bin/env bash
exec node "$repo_root/languages/typescript-lang-project-harness/dist/src/cli/main.js" "\$@"
SH

cat >"$shim_dir/py-harness" <<SH
#!/usr/bin/env bash
exec uv run --project "$repo_root/languages/python-lang-project-harness" --frozen py-harness "\$@"
SH

chmod +x "$shim_dir/rs-harness" "$shim_dir/ts-harness" "$shim_dir/py-harness"

export PATH="$shim_dir:$PATH"
export SEMANTIC_AGENT_PROTOCOL_BIN="$repo_root/target/debug/asp"

bash tools/validate-language-tree-sitter-runtime-boundary.sh
bash tools/validate-tree-sitter-frontier-code-contract.sh
bash tools/validate-search-read-plan-frontier-contract.sh
bash tools/validate-exact-direct-read-contract.sh
uv run --project packages/python python -m tools tree-sitter validate rust-query-corpus
uv run --project packages/python python -m tools tree-sitter validate typescript-query-corpus
uv run --project packages/python python -m tools tree-sitter validate python-query-corpus
uv run --project packages/python python -m tools tree-sitter validate json-abi-corpus

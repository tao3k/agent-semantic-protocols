set shell := ["bash", "-cu"]

repo := "."
rust_harness_project := "languages/rust-lang-project-harness"
typescript_harness_project := "languages/typescript-lang-project-harness"
python_harness_project := "languages/python-lang-project-harness"
julia_harness_project := "languages/JuliaLangProjectHarness.jl"
julia_harness := "julia --project=languages/JuliaLangProjectHarness.jl languages/JuliaLangProjectHarness.jl/bin/julia-project-harness.jl"

default:
    @just --list

# Install all agent tools and Codex hook config. Optional: just agent-hooks-install ~/.local/bin
agent-hooks-install bin_dir="":
    @bin_dir="{{bin_dir}}"; \
      if [ -z "${bin_dir}" ]; then bin_dir="${SEMANTIC_AGENT_BIN_DIR:-$HOME/.local/bin}"; fi; \
      just agent-tools-install-global "${bin_dir}"; \
      just agent-hooks-install-current "${bin_dir}"; \
      just agent-hooks-doctor "${bin_dir}"

agent-hooks-install-current bin_dir="":
    @just agent-hooks-install-root "{{bin_dir}}"

agent-hooks-doctor bin_dir="":
    @just agent-hooks-doctor-root "{{bin_dir}}"

agent-hooks-install-root bin_dir="":
    @bin_dir="{{bin_dir}}"; \
      if [ -z "${bin_dir}" ]; then bin_dir="${SEMANTIC_AGENT_BIN_DIR:-}"; fi; \
      if [ -n "${bin_dir}" ]; then \
        protocol_bin="${bin_dir}/asp"; \
        if [ -x "${protocol_bin}" ]; then \
          "${protocol_bin}" hook install --client codex {{repo}}; \
        else \
          cargo run -q -p agent-semantic-protocol --bin asp -- hook install --client codex {{repo}}; \
        fi; \
      else \
        cargo run -q -p agent-semantic-protocol --bin asp -- hook install --client codex {{repo}}; \
      fi

agent-hooks-doctor-root bin_dir="":
    @bin_dir="{{bin_dir}}"; \
      if [ -z "${bin_dir}" ]; then bin_dir="${SEMANTIC_AGENT_BIN_DIR:-}"; fi; \
      if [ -n "${bin_dir}" ]; then \
        protocol_bin="${bin_dir}/asp"; \
        if [ -x "${protocol_bin}" ]; then \
          "${protocol_bin}" hook doctor --client codex {{repo}}; \
        else \
          cargo run -q -p agent-semantic-protocol --bin asp -- hook doctor --client codex {{repo}}; \
        fi; \
      else \
        cargo run -q -p agent-semantic-protocol --bin asp -- hook doctor --client codex {{repo}}; \
      fi

# Replay the root classifier directly without launching Codex.
agent-hooks-smoke-hook:
    @printf '%s' '{"tool_name":"functions.exec_command","tool_input":{"cmd":"sed -n '\''1,8p'\'' languages/typescript-lang-project-harness/tests/unit/cli.test.ts"}}' \
      | cargo run -q -p agent-semantic-protocol --bin asp -- hook pre-tool --client codex --activation .cache/agent-semantic-protocol/hooks/activation.json --config .codex/agent-semantic-protocol/hooks/config.toml --emit decision \
      | python3 -c 'import json,sys; d=json.load(sys.stdin); assert d["decision"]=="deny", d; assert d["reasonKind"] in {"bulk-source-dump","direct-source-read"}, d; print("[agent-hooks-smoke-hook] blocked", d["reasonKind"])'

# Launch Codex CLI and verify the real PreToolUse runtime blocks a TS source dump.
agent-hooks-smoke-codex:
    @out="$(mktemp)"; \
      codex_bin="$(command -v codex || true)"; \
      if [ -z "${codex_bin}" ] && [ -x /Applications/Codex.app/Contents/Resources/codex ]; then codex_bin=/Applications/Codex.app/Contents/Resources/codex; fi; \
      if [ -z "${codex_bin}" ]; then echo "codex binary not found on PATH"; rm -f "${out}"; exit 127; fi; \
      "${codex_bin}" exec --json --dangerously-bypass-approvals-and-sandbox --dangerously-bypass-hook-trust -C "$PWD" \
        "Run exactly this shell command and do nothing else: sed -n '1,8p' languages/typescript-lang-project-harness/tests/unit/cli.test.ts" >"${out}" 2>&1 || true; \
      if rg -q "Command blocked by PreToolUse hook: bulk-source-dump denied|permissionDecision.*deny" "${out}"; then \
        echo "[agent-hooks-smoke-codex] blocked"; \
      else \
        cat "${out}"; \
        rm -f "${out}"; \
        exit 1; \
      fi; \
      rm -f "${out}"

# Install asp, rs-harness, ts-harness, and py-harness.
agent-tools-install-global bin_dir="":
    @bin_dir="{{bin_dir}}"; \
      if [ -z "${bin_dir}" ]; then bin_dir="${SEMANTIC_AGENT_BIN_DIR:-$HOME/.local/bin}"; fi; \
      just agent-tools-install-protocol "${bin_dir}"; \
      just agent-tools-install-hook "${bin_dir}"; \
      just agent-tools-install-rs "${bin_dir}"; \
      just agent-tools-install-ts "${bin_dir}"; \
      just agent-tools-install-py "${bin_dir}"; \
      echo "[agent-tools-install-global] installed asp, rs-harness, ts-harness, and py-harness into ${bin_dir}"

# Install only the shared asp binary.
agent-tools-install-protocol bin_dir="":
    @bin_dir="{{bin_dir}}"; \
      if [ -z "${bin_dir}" ]; then bin_dir="${SEMANTIC_AGENT_BIN_DIR:-$HOME/.local/bin}"; fi; \
      mkdir -p "${bin_dir}"; \
      cargo build --release --manifest-path Cargo.toml --package agent-semantic-protocol --bin asp; \
      install -m 755 target/release/asp "${bin_dir}/asp"; \
      test -x "${bin_dir}/asp"; \
      "${bin_dir}/asp" guide >/dev/null

# Install the shared protocol binary used by hook runtime commands.
agent-tools-install-hook bin_dir="":
    @just agent-tools-install-protocol "{{bin_dir}}"

# Install only the Rust provider binary.
agent-tools-install-rust bin_dir="":
    @just agent-tools-install-rs "{{bin_dir}}"

agent-tools-install-rs bin_dir="":
    @bin_dir="{{bin_dir}}"; \
      if [ -z "${bin_dir}" ]; then bin_dir="${SEMANTIC_AGENT_BIN_DIR:-$HOME/.local/bin}"; fi; \
      mkdir -p "${bin_dir}"; \
      cargo build --release --manifest-path {{rust_harness_project}}/Cargo.toml --features cli --bin rs-harness; \
      install -m 755 {{rust_harness_project}}/target/release/rs-harness "${bin_dir}/rs-harness"; \
      "${bin_dir}/rs-harness" --help >/dev/null

# Install only the TypeScript provider binary.
agent-tools-install-typescript bin_dir="":
    @just agent-tools-install-ts "{{bin_dir}}"

agent-tools-install-ts bin_dir="":
    @bin_dir="{{bin_dir}}"; \
      if [ -z "${bin_dir}" ]; then bin_dir="${SEMANTIC_AGENT_BIN_DIR:-$HOME/.local/bin}"; fi; \
      mkdir -p "${bin_dir}"; \
      npm --prefix {{typescript_harness_project}} run -s build >/dev/null; \
      rm -f "${bin_dir}/ts-harness"; \
      printf '#!/usr/bin/env bash\nexec node "%s/%s/dist/src/cli/main.js" "$@"\n' "$PWD" "{{typescript_harness_project}}" > "${bin_dir}/ts-harness"; \
      chmod 755 "${bin_dir}/ts-harness"; \
      "${bin_dir}/ts-harness" --help >/dev/null

# Install only the Python provider binary.
agent-tools-install-python bin_dir="":
    @just agent-tools-install-py "{{bin_dir}}"

agent-tools-install-py bin_dir="":
    @bin_dir="{{bin_dir}}"; \
      if [ -z "${bin_dir}" ]; then bin_dir="${SEMANTIC_AGENT_BIN_DIR:-$HOME/.local/bin}"; fi; \
      mkdir -p "${bin_dir}"; \
      uv tool install --force --editable {{python_harness_project}}; \
      py_bin="$(uv tool dir --bin)/py-harness"; \
      ln -sfn "${py_bin}" "${bin_dir}/py-harness"; \
      "${bin_dir}/py-harness" --help >/dev/null

agent-hooks-doctor-providers: agent-hooks-doctor-rs agent-hooks-doctor-ts agent-hooks-doctor-py

agent-hooks-doctor-rs:
    rs-harness agent doctor {{repo}}

agent-hooks-doctor-ts:
    ts-harness agent doctor {{repo}}

agent-hooks-doctor-py:
    py-harness agent doctor {{repo}}

check-sandtables:
    uv run semantic-sandtable

provider-gate: check-rust-warnings provider-gate-root provider-gate-rust provider-gate-typescript provider-gate-python provider-gate-julia

check-rust-warnings:
    direnv exec . env RUSTFLAGS="-D warnings" cargo check -q -p agent-semantic-protocol
    direnv exec . env RUSTFLAGS="-D warnings" cargo check -q --manifest-path {{rust_harness_project}}/Cargo.toml --features cli,search

provider-gate-root:
    direnv exec . cargo test -p agent-semantic-hook
    direnv exec . python -m pytest tests/unit/test_semantic_*_schema.py tests/unit/semantic_sandtable

provider-gate-rust:
    direnv exec . cargo test --manifest-path {{rust_harness_project}}/Cargo.toml --features cli,search search
    direnv exec . cargo test --manifest-path {{rust_harness_project}}/Cargo.toml --features cli,search query
    direnv exec . cargo test --manifest-path {{rust_harness_project}}/Cargo.toml --features cli,search policy

provider-gate-typescript:
    direnv exec . npm --prefix {{typescript_harness_project}} run build
    direnv exec . npm --prefix {{typescript_harness_project}} run check:implementation
    direnv exec . node --test \
      {{typescript_harness_project}}/dist/tests/unit/cli_compact_query_snapshot.test.js \
      {{typescript_harness_project}}/dist/tests/unit/cli_ast_patch.test.js \
      {{typescript_harness_project}}/dist/tests/unit/cli_item_query.test.js \
      {{typescript_harness_project}}/dist/tests/unit/cli_item_query_code.test.js \
      {{typescript_harness_project}}/dist/tests/unit/cli_item_query_fallback.test.js \
      {{typescript_harness_project}}/dist/tests/unit/cli_search_policy.test.js \
      {{typescript_harness_project}}/dist/tests/unit/cli_search_query.test.js \
      {{typescript_harness_project}}/dist/tests/unit/semantic_language_registry_read_packet.test.js \
      {{typescript_harness_project}}/dist/tests/unit/semantic_search_registry_expectations.js \
      {{typescript_harness_project}}/dist/tests/unit/semantic_search_schema.test.js

provider-gate-python:
    direnv exec . uv run --project {{python_harness_project}} --frozen py-harness search policy PY-PROJ-R001 owner tests --view seeds {{python_harness_project}}
    direnv exec . uv run --project {{python_harness_project}} --frozen py-harness search policy PY-AGENT-R008 owner tests --view seeds {{python_harness_project}}
    direnv exec . uv run --project {{python_harness_project}} --frozen py-harness query src/python_lang_project_harness/_semantic_language.py --term semantic_language_registry_document --names-only {{python_harness_project}}
    direnv exec . uv run --project {{python_harness_project}} --frozen python -m pytest \
      {{python_harness_project}}/tests/unit/harness/test_semantic_cli_query_set.py \
      {{python_harness_project}}/tests/unit/harness/test_semantic_cli_owner_items.py \
      {{python_harness_project}}/tests/unit/harness/test_semantic_cli_policy.py \
      {{python_harness_project}}/tests/unit/harness/test_semantic_schema_registry.py

provider-gate-julia:
	direnv exec . julia --project={{julia_harness_project}} -e 'using Pkg; Pkg.test()'
	@echo "[provider-gate-julia] skipped CLI runtime smoke; Julia startup/warmup is tracked separately"

check-python-policy:
    uv run --project {{python_harness_project}} --frozen py-harness check --full {{repo}}

report-python-policy:
    uv run --project {{python_harness_project}} --frozen py-harness check --full {{repo}} || true

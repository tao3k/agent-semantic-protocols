# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

set shell := ["bash", "-cu"]

repo := "."
asp_rust_project := "languages/asp-rust"
asp_typescript_project := "languages/asp-typescript"
asp_python_project := "languages/asp-python"
asp_julia_project := "languages/AspJulia.jl"
asp_julia_command := "julia --project=languages/AspJulia.jl languages/AspJulia.jl/bin/asp-julia.jl"
asp_julia_compiled_command := "languages/AspJulia.jl/build/juliac-asp-local/asp-julia"
asp_gerbil_scheme_project := "languages/asp-gerbil-scheme"
asp_state_home := env_var_or_default("ASP_STATE_HOME", home_directory() / ".agent-semantic-protocols")
asp_runtime_bin := asp_state_home / "runtime" / "bin"

default:
	@just --list

# Fast profile-native shell; only devenv input drift invokes direnv evaluation.
[positional-arguments]
devenv +args:
	@scripts/devenv-profile-exec.sh "$@"

_agent-tools-run-asp bin_dir +args:
    @bin_dir="{{bin_dir}}"; \
    if [ -z "${bin_dir}" ]; then bin_dir="${SEMANTIC_AGENT_BIN_DIR:-{{asp_runtime_bin}}}"; fi; \
    case ":$PATH:" in *":${bin_dir}:"*) ;; *) PATH="${bin_dir}:$PATH"; export PATH ;; esac; \
    protocol_bin="${ASP_BIN:-${bin_dir}/asp}"; \
    if [ -x "${protocol_bin}" ]; then \
      stale_reason=""; \
      if [ -z "${ASP_BIN:-}" ]; then \
        if [ -x target/release/asp ] && [ target/release/asp -nt "${protocol_bin}" ]; then \
          stale_reason="target/release/asp is newer than ${protocol_bin}"; \
        elif [ -d crates/agent-semantic-protocol/src ] && [ -n "$(find crates/agent-semantic-protocol/src -type f -newer "${protocol_bin}" -print -quit)" ]; then \
          stale_reason="agent-semantic-protocol Rust source is newer than ${protocol_bin}"; \
        fi; \
      fi; \
      if [ -n "${stale_reason}" ]; then \
        echo "[agent-tools-run-asp] stale ${protocol_bin}: ${stale_reason}; run \`just agent-tools-install-protocol ${bin_dir}\`" >&2; \
        exit 1; \
      fi; \
      SEMANTIC_AGENT_BIN_DIR="${bin_dir}" "${protocol_bin}" {{args}}; \
    else \
      SEMANTIC_AGENT_BIN_DIR="${bin_dir}" cargo run -q -p agent-semantic-client --bin asp -- {{args}}; \
    fi

# Develop mode: install this checkout's tools and Codex hooks.
install bin_dir="":
	@just agent-tools-ensure-local-bin-path
	@just agent-hooks-install "{{bin_dir}}"

# Publish only the canonical ASP entry into the user PATH.
agent-tools-ensure-local-bin-path:
    @canonical_runtime_asp="{{ asp_runtime_bin }}/asp"; \
      user_path_dir="$HOME/.local/bin"; \
      user_path_entry="${user_path_dir}/asp"; \
      test -x "${canonical_runtime_asp}"; \
      mkdir -p "${user_path_dir}"; \
      temporary_link="${user_path_entry}.tmp.$$"; \
      ln -s "${canonical_runtime_asp}" "${temporary_link}"; \
      mv -f "${temporary_link}" "${user_path_entry}"; \
      echo "[agent-tools-ensure-local-bin-path] canonicalRuntimeAsp=${canonical_runtime_asp} userPathEntry=${user_path_entry} switch=atomic"

# Install the ASP runtime, language providers, and Codex hook config under ASP State Home.
agent-hooks-install bin_dir="":
	@bin_dir="{{bin_dir}}"; \
	if [ -z "${bin_dir}" ]; then bin_dir="${SEMANTIC_AGENT_BIN_DIR:-{{asp_runtime_bin}}}"; fi; \
	just agent-tools-install-global "${bin_dir}"; \
	just _agent-hooks-install-codex "${bin_dir}"; \
	just agent-hooks-doctor "${bin_dir}"

agent-hooks-doctor bin_dir="":
	@just _agent-hooks-doctor-codex "{{bin_dir}}"

_agent-hooks-install-codex bin_dir="":
	@just _agent-tools-run-asp "{{bin_dir}}" install plugin --codex {{repo}}

_agent-hooks-doctor-codex bin_dir="":
	@just _agent-tools-run-asp "{{bin_dir}}" hook doctor --client codex {{repo}}

# Replay the root classifier directly without launching Codex.
agent-hooks-smoke-hook:
    @activation="$(cargo run -q -p agent-semantic-client --bin asp -- hook paths . | awk -F= '$1=="activation"{print substr($0, 12)}')"; \
      printf '%s' '{"tool_name":"functions.exec_command","tool_input":{"cmd":"sed -n '\''1,8p'\'' languages/asp-typescript/tests/unit/cli.test.ts"}}' \
      | cargo run -q -p agent-semantic-client --bin asp -- hook pre-tool --client codex --activation "$activation" --config .codex/agent-semantic-protocol/hooks/config.toml --emit decision \
      | python3 -c 'import json,sys; d=json.load(sys.stdin); assert d["decision"]=="deny", d; assert d["reasonKind"] in {"bulk-source-dump","direct-source-read"}, d; print("[agent-hooks-smoke-hook] blocked", d["reasonKind"])'

# Launch Codex CLI and verify the real PreToolUse runtime blocks a TS source dump.
agent-hooks-smoke-codex:
    @out="$(mktemp)"; \
      codex_bin="$(command -v codex || true)"; \
      if [ -z "${codex_bin}" ] && [ -x /Applications/Codex.app/Contents/Resources/codex ]; then codex_bin=/Applications/Codex.app/Contents/Resources/codex; fi; \
      if [ -z "${codex_bin}" ]; then echo "codex binary not found on PATH"; rm -f "${out}"; exit 127; fi; \
      "${codex_bin}" exec --json --dangerously-bypass-approvals-and-sandbox --dangerously-bypass-hook-trust -C "$PWD" \
        "Run exactly this shell command and do nothing else: sed -n '1,8p' languages/asp-typescript/tests/unit/cli.test.ts" >"${out}" 2>&1 || true; \
      if rg -q "Command blocked by PreToolUse hook: bulk-source-dump denied|permissionDecision.*deny" "${out}"; then \
        echo "[agent-hooks-smoke-codex] blocked"; \
      elif rg -q '"type":"command_execution"' "${out}"; then \
        echo "[agent-hooks-smoke-codex] unsupported-surface=command_execution; PreToolUse only intercepts Bash, apply_patch, and MCP tools"; \
        rm -f "${out}"; \
        exit 2; \
      else \
        cat "${out}"; \
        rm -f "${out}"; \
        exit 1; \
      fi; \
      rm -f "${out}"

# Develop mode: build and install asp plus all providers from this checkout.
agent-tools-install-global bin_dir="":
    @bin_dir="{{bin_dir}}"; \
    if [ -z "${bin_dir}" ]; then bin_dir="${SEMANTIC_AGENT_BIN_DIR:-{{asp_runtime_bin}}}"; fi; \
      just agent-tools-install-protocol "${bin_dir}"; \
      just agent-tools-install-languages; \
      echo "[agent-tools-install-global] installed asp and all ASP language providers; asp-python-graphs remains an ASP Server-owned runtime service"

# Develop mode: build and install the shared asp binary with the embedded Orgize provider.
agent-tools-install-orgize bin_dir="":
    @just agent-tools-install-asp "{{bin_dir}}"

# Develop mode: build and install all language providers from this checkout.
agent-tools-install-languages:
    @just agent-tools-install-rs
    @just agent-tools-install-ts
    @just agent-tools-install-py
    @just agent-tools-install-julia
    @just agent-tools-install-gerbil
    @echo "[agent-tools-install-languages] installed asp-rust, asp-typescript, asp-python, asp-julia, and asp-gerbil-scheme into {{asp_runtime_bin}}"

# Develop mode: build and install the shared asp binary from this checkout.
agent-tools-install-asp bin_dir="":
	@just agent-tools-install-protocol "{{bin_dir}}"

# Build the ASP release binary without coupling it to provider runtime artifacts.
build-asp-release:
    cargo build --release --manifest-path Cargo.toml --package agent-semantic-client --bin asp

agent-tools-install-protocol bin_dir="": check-rust-workspace-policy
    @requested_bin_dir="{{bin_dir}}"; \
      if [ -n "${requested_bin_dir}" ]; then \
        echo "agent-tools-install-protocol no longer accepts a custom bin_dir; Runtime configuration owns the stable install slot" >&2; \
        exit 2; \
      fi; \
      cargo_target_dir="${CARGO_TARGET_DIR:-target}"; \
      asp_artifact="${cargo_target_dir}/release/asp"; \
      cargo build --release --manifest-path Cargo.toml --package agent-semantic-client --bin asp || exit $?; \
      "${asp_artifact}" --version --require-release >/dev/null; \
      destination="$("${asp_artifact}" paths --get runtimeBinDir)/asp"; \
      "${asp_artifact}" install binary || exit $?; \
      test -x "${destination}"; \
      "${destination}" --version --require-release >/dev/null

# Install the debug protocol binary into the canonical State Home runtime.
# Developer publication keeps member build scripts O(1), then admits the
# complete Cargo-derived workspace policy exactly once before publishing.
# ASP Rust owns content-addressed package evidence, so unchanged packages are
# reused instead of being rescanned by every downstream build script.
agent-tools-install-protocol-debug: check-rust-workspace-policy
    @asp_artifact="target/debug/asp"; \
      cargo build --manifest-path Cargo.toml --package agent-semantic-client --bin asp --package agent-semantic-hook --bin asp-hook || exit $?; \
      destination="$("${asp_artifact}" paths --get runtimeBinDir)/asp"; \
      "${asp_artifact}" install binary || exit $?; \
      test -x "${destination}"

# Install the shared protocol binary used by hook runtime commands.
agent-tools-install-hook bin_dir="":
	@just agent-tools-install-protocol "{{bin_dir}}"

# Install a language provider through ASP's registered workspace-install contract.
# The root Justfile is only an adapter; provider-owned descriptors own builds and artifacts.
agent-tools-install-language language target="":
    #!/usr/bin/env bash
    set -euo pipefail
    state_home="{{asp_state_home}}"
    target="{{ target }}"
    install_args=(
      install language "{{ language }}"
    )
    if [[ -n "${target}" ]]; then
      install_args+=(--target "${target}")
    fi
    protocol_bin="${state_home}/runtime/bin/asp"
    if [[ ! -x "${protocol_bin}" ]]; then
      echo "canonical ASP binary is required to install the provider; run 'just agent-tools-install-protocol' first" >&2
      exit 1
    fi
    "${protocol_bin}" "${install_args[@]}"
    echo "[agent-tools-install] language={{ language }} installMode=provider-workspace-install source=provider-registry receipt=recorded"

# Develop mode: build and install the Rust provider from this checkout.
agent-tools-install-rust:
    @just agent-tools-install-rs

agent-tools-install-rs:
    @just agent-tools-install-language rust

# Develop mode: build and install the TypeScript provider from this checkout.
agent-tools-install-typescript:
    @just agent-tools-install-ts

agent-tools-install-ts:
    @just agent-tools-install-language typescript

# Develop mode: build and install the Python provider from this checkout.
agent-tools-install-python:
    @just agent-tools-install-py

agent-tools-install-py:
    @just agent-tools-install-language python

# Develop mode: build and install the Julia provider from this checkout.
agent-tools-install-julia bin_dir="":
    @just agent-tools-install-jl "{{bin_dir}}"

agent-tools-install-jl bin_dir="":
    @set -e; \
      state_home="{{asp_state_home}}"; \
      ASP_JULIA_ALLOW_WRAPPER_FALLBACK=0 just agent-tools-install-language julia; \
      if [ -n "{{bin_dir}}" ]; then \
        mkdir -p "{{bin_dir}}"; \
        provider_bin="$${state_home}/runtime/bin/asp-julia"; \
        test -x "$${provider_bin}"; \
        install -m 755 "$${provider_bin}" "{{bin_dir}}/asp-julia"; \
      fi; \
      echo "[agent-tools-install-julia] provider=asp-julia source=provider-registry fallback=disabled"

# Develop mode: build and install the Gerbil Scheme provider from this checkout.
agent-tools-install-gerbil:
    @just agent-tools-install-language gerbil-scheme

agent-tools-build-gerbil bin_dir="":
    @set -e; \
      repo_root="$PWD"; \
      package_dir="${repo_root}/{{asp_gerbil_scheme_project}}"; \
      artifact_root="${package_dir}/build/workspace-provider"; \
      cd "${package_dir}"; \
      gxpkg env gxi ./build-provider.ss compile; \
      provider_binary="${artifact_root}/bin/asp-gerbil-scheme"; \
      test -x "${provider_binary}"; \
      if [ -n "{{bin_dir}}" ]; then \
        mkdir -p "{{bin_dir}}"; \
        cp "${provider_binary}" "{{bin_dir}}/asp-gerbil-scheme"; \
      fi; \
      echo "[agent-tools-build] provider=asp-gerbil-scheme artifactRoot=${artifact_root} binary=${provider_binary}"

test-gerbil-provider-http-json: agent-tools-build-gerbil
    @set -e; \
      repo_root="$PWD"; \
      package_dir="${repo_root}/{{asp_gerbil_scheme_project}}"; \
      cd "${package_dir}"; \
      GERBIL_PATH="${package_dir}/.gerbil" \
        gxtest t/projection-batch-test.ss; \
      GERBIL_PATH="${package_dir}/.gerbil" \
        gxtest t/provider-http-json-server-test.ss

agent-tools-install-gx bin_dir="":
    @just agent-tools-build-gerbil "{{bin_dir}}"

agent-hooks-doctor-providers: agent-hooks-doctor-rs agent-hooks-doctor-ts agent-hooks-doctor-py agent-hooks-doctor-julia

agent-hooks-doctor-rs:
    asp-rust agent doctor {{repo}}

agent-hooks-doctor-ts:
    asp-typescript agent doctor {{repo}}

agent-hooks-doctor-py:
    asp-python agent doctor {{repo}}

agent-hooks-doctor-julia:
    asp-julia agent doctor --json {{asp_julia_project}} >/dev/null

check-sandtables:
    uv run --project packages/python python -m tools sandtable

benchmark-large-library-search-runtime:
    test -n "${ASP_STATE_HOME:-}"; direnv exec . env ASP_BENCHMARK_BIN="$PWD/target/release/asp" uv run --project packages/python --frozen --exact python -m tools.semantic_sandtable --repo-root . --large-library-runtime-benchmark --large-library-runtime-asp-bin target/release/asp --large-library-runtime-state-home "$ASP_STATE_HOME"

benchmark-large-library-search-runtime-baseline:
    test -n "${ASP_STATE_HOME:-}"; receipt="$PWD/.cache/large-library-runtime-search.v1.receipt.json"; mkdir -p "$(dirname "$receipt")"; set +e; direnv exec . env ASP_BENCHMARK_BIN="$PWD/target/release/asp" uv run --project packages/python --frozen --exact python -m tools.semantic_sandtable --repo-root . --json --large-library-runtime-benchmark --large-library-runtime-asp-bin target/release/asp --large-library-runtime-state-home "$ASP_STATE_HOME" > "$receipt"; runtime_status=$?; set -e; direnv exec . env ASP_BENCHMARK_BIN="$PWD/target/release/asp" uv run --project packages/python --frozen --exact python -m tools.semantic_sandtable.large_library_runtime_baseline --baseline benchmarks/large-library-runtime-search.v1.baseline.json --receipt "$receipt"; baseline_status=$?; test "$runtime_status" -eq 0; test "$baseline_status" -eq 0

check-graph-turbo-focused:
    uv run --project packages/python/asp_python_graphs --frozen pytest \
      tests/unit/test_asp_graph_turbo_request.py \
      tests/unit/test_asp_graph_turbo_feedback.py \
      tests/unit/test_asp_graph_turbo_calibration.py \
      tests/unit/test_asp_graph_turbo_projection_fields.py \
      tests/unit/test_asp_graph_turbo_ranking_collection_fields.py \
      tests/unit/test_asp_graph_turbo_read_loop.py \
      tests/unit/test_asp_graph_turbo_timeline.py \
      tests/unit/test_asp_graph_turbo_timeline_text.py \
      tests/unit/semantic_sandtable/test_agent_observation_flow.py \
      tests/unit/semantic_sandtable/test_agent_observation_read_loop.py \
      tests/unit/semantic_sandtable/test_expectations.py

check-language-facade-smoke:
    uv run --project packages/python/asp_python_graphs --frozen pytest tests/unit/test_language_facade_smoke.py -q

check-provider-knowledge-axes:
    node tools/provider-knowledge-axes-close-loop.mjs

# Qualify every locked large-library corpus through resident search, exact projection, and OTel.
check-live-corpus-search-query-all-setup:
    just agent-tools-install-protocol
    just agent-tools-install-rs
    just agent-tools-install-ts
    just agent-tools-install-py
    just agent-tools-install-julia
    just agent-tools-install-orgize

check-live-corpus-search-query-all: check-live-corpus-search-query-all-setup
    "{{asp_runtime_bin}}/asp" server start >/dev/null
    "{{asp_runtime_bin}}/asp" live-corpus qualify --plan benchmarks/live-corpus-search-query-qualification.json

provider-gate: check-rust-warnings check-schema-profiles check-rfc-docs check-rust-workspace-policy check-schema-manager check-tree-sitter-query-contracts check-graph-turbo-focused provider-gate-root provider-gate-rust provider-gate-typescript provider-gate-python provider-gate-julia provider-gate-gerbil

# Run the parser-owned whole-workspace policy exactly once. Ordinary member
# builds keep the O(1) manifest/policy-identity dependency and never rescan the
# crate source tree.
check-rust-workspace-policy:
    rtk cargo test -p asp-rust-project-harness-policy --features workspace-policy --test integration_test workspace_policy::asp_workspace_source_policy_is_clean -- --exact --nocapture

check-rust-warnings:
    env RUSTFLAGS="-D warnings" cargo check -q -p agent-semantic-client
    env RUSTFLAGS="-D warnings" cargo check -q --manifest-path {{asp_rust_project}}/Cargo.toml --all-features

check-schema-profiles:
    rtk cargo run --quiet -p agent-semantic-schema-manager -- verify --workspace .

check-schema-manager: check-schema-proof-plan
	uv run --project packages/python --frozen --exact asp-schema-manager check --workspace-root . --fail-on-family-local-refs --fail-on-unclassified-schemas --fail-on-mixed-family-refs --fail-on-reference-decision-drift

check-schema-proof-plan:
    uv run --project packages/python --frozen --exact pytest packages/python/asp_schema_manager/tests/unit/test_logical_projection.py packages/python/asp_schema_manager/tests/unit/test_proof_plan_cli.py -q

report-schema-manager:
    uv run --project packages/python --frozen --exact asp-schema-manager audit --workspace-root .

check-tree-sitter-query-contracts:
    uv run --project packages/python --frozen --exact python -m tools tree-sitter validate contracts

check-rfc-docs:
    uv run --project packages/python --frozen --exact pytest \
      tests/unit/test_*rfc.py \
      tests/unit/test_docs_rfc_skill_contracts.py \
      -q

provider-gate-root: check-language-facade-smoke
    just check-gerbil-owner-items-fast-path
    cargo test -p agent-semantic-hook
    uv run --project packages/python --frozen --exact python -m pytest \
      tests/unit/test_semantic_*_schema.py \
      tests/unit/semantic_tree_sitter_query_rfc \
      tests/unit/test_asp_server_first_architecture_rfc.py \
      tests/unit/test_agent_hook_interception_protocol_rfc.py \
      tests/unit/test_docs_rfc_skill_contracts.py \
      tests/unit/test_python_package_dependency_boundary.py \
      tests/unit/semantic_sandtable

check-gerbil-owner-items-fast-path:
    #!/usr/bin/env python3
    import os
    import statistics
    import subprocess
    import sys
    import time
    from pathlib import Path

    root = Path.cwd()
    asp_bin = Path(os.environ.get("ASP_BIN", root / ".bin" / "asp"))
    if not asp_bin.is_file() or not os.access(asp_bin, os.X_OK):
        raise SystemExit(
            f"[gerbil-owner-items-fast] missing executable {asp_bin}; "
            "run `just agent-tools-install-protocol .bin`"
        )

    profile = subprocess.run(
        [str(asp_bin), "--version", "--require-release"],
        cwd=root,
        text=True,
        capture_output=True,
    )
    if profile.returncode != 0:
        sys.stderr.write(profile.stderr)
        raise SystemExit(profile.returncode)

    command = [
        str(asp_bin),
        "gerbil-scheme",
        "search",
        "owner",
        "build.ss",
        "items",
        "--query",
        "build-spec release cli-launcher make parallelize build-release build-optimized",
        "--workspace",
        "{{asp_gerbil_scheme_project}}",
        "--view",
        "seeds",
    ]
    max_seconds = float(os.environ.get("ASP_GERBIL_OWNER_ITEMS_MAX_SECONDS", "0.25"))
    runs = int(os.environ.get("ASP_GERBIL_OWNER_ITEMS_RUNS", "5"))
    if runs < 1:
        raise SystemExit("[gerbil-owner-items-fast] ASP_GERBIL_OWNER_ITEMS_RUNS must be >= 1")

    durations: list[float] = []
    for _ in range(runs):
        started = time.perf_counter()
        proc = subprocess.run(command, cwd=root, text=True, capture_output=True)
        elapsed = time.perf_counter() - started
        if proc.returncode != 0:
            sys.stderr.write(proc.stderr)
            sys.stderr.write(proc.stdout)
            raise SystemExit(proc.returncode)
        stdout = proc.stdout
        if "reason=rust-inline-gerbil-owner-items" not in stdout:
            sys.stderr.write(stdout)
            raise SystemExit(
                "[gerbil-owner-items-fast] expected Rust inline owner-items path; "
                "no fallback to Gerbil provider is allowed"
            )
        if "source=rust-inline" not in stdout:
            sys.stderr.write(stdout)
            raise SystemExit("[gerbil-owner-items-fast] expected Rust inline item source")
        durations.append(elapsed)

    median = statistics.median(durations)
    max_seen = max(durations)
    print(
        "[gerbil-owner-items-fast] "
        f"runs={runs} median={median:.3f}s max={max_seen:.3f}s threshold={max_seconds:.3f}s"
    )
    if median > max_seconds:
        raise SystemExit(
            "[gerbil-native-syntax-playbook] median latency exceeded threshold; "
            "keep native syntax inside the single Search playbook generation path"
        )

provider-gate-rust:
    cargo test --manifest-path {{asp_rust_project}}/Cargo.toml --features provider-server parser_native_syntax
    cargo test --manifest-path {{asp_rust_project}}/Cargo.toml --features provider-server project_resolution
    cargo test --manifest-path {{asp_rust_project}}/Cargo.toml --features provider-server policy

provider-gate-typescript:
    npm --prefix {{asp_typescript_project}} run build
    npm --prefix {{asp_typescript_project}} run check:implementation
    node --test \
      {{asp_typescript_project}}/dist/tests/unit/cli_compact_query_snapshot.test.js \
      {{asp_typescript_project}}/dist/tests/unit/cli_ast_patch.test.js \
      {{asp_typescript_project}}/dist/tests/unit/cli_item_query.test.js \
      {{asp_typescript_project}}/dist/tests/unit/cli_item_query_code.test.js \
      {{asp_typescript_project}}/dist/tests/unit/cli_item_query_fallback.test.js \
      {{asp_typescript_project}}/dist/tests/unit/semantic_language_registry_read_packet.test.js

provider-gate-python:
    uv run --project {{asp_python_project}} --frozen python -m pytest \
      {{asp_python_project}}/tests/unit/asp_python/test_semantic_cli_query_set.py \
      {{asp_python_project}}/tests/unit/asp_python/test_semantic_schema_registry.py

provider-gate-julia:
	julia --project={{asp_julia_project}} -e 'using Pkg; Pkg.test()'
	{{asp_julia_command}} guide {{asp_julia_project}} >/dev/null
	{{asp_julia_command}} agent doctor --json {{asp_julia_project}} >/dev/null
	{{asp_julia_compiled_command}} guide {{asp_julia_project}} >/dev/null
	{{asp_julia_compiled_command}} agent doctor --json {{asp_julia_project}} >/dev/null
	just check-language-facade-smoke

provider-gate-gerbil:
    just test-gerbil-provider-http-json

# Validate repository SPDX/REUSE coverage and package license metadata.
check-license-contract:
    uv run --frozen python scripts/check_license_contract.py
    uv run --frozen reuse --root . lint

report-python-policy:
    uv run --project {{asp_python_project}} --frozen python -c 'from asp_python import render_asp_python_report, run_asp_python; print(render_asp_python_report(run_asp_python("{{repo}}")), end="")'
# Develop mode: build and install the debug asp binary from this checkout.
agent-tools-install-asp-dev:
    @just agent-tools-install-protocol-debug

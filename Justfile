set shell := ["bash", "-cu"]

repo := "."
rust_harness_project := "languages/rust-lang-project-harness"
typescript_harness_project := "languages/typescript-lang-project-harness"
python_harness_project := "languages/python-lang-project-harness"
julia_harness_project := "languages/JuliaLangProjectHarness.jl"
julia_harness := "julia --project=languages/JuliaLangProjectHarness.jl languages/JuliaLangProjectHarness.jl/bin/julia-project-harness.jl"
julia_compiled_harness := "languages/JuliaLangProjectHarness.jl/build/juliac-asp-local/asp-julia-harness"
gerbil_harness_project := "languages/gerbil-scheme-language-project-harness"

default:
	@just --list

_agent-tools-run-asp bin_dir +args:
    @bin_dir="{{bin_dir}}"; \
    if [ -z "${bin_dir}" ]; then bin_dir="${SEMANTIC_AGENT_BIN_DIR:-$HOME/.local/bin}"; fi; \
    case ":$PATH:" in *":${bin_dir}:"*) ;; *) PATH="${bin_dir}:$PATH"; export PATH ;; esac; \
    protocol_bin="${ASP_BIN:-${bin_dir}/asp}"; \
    if [ -x "${protocol_bin}" ]; then \
      stale_reason=""; \
      if [ -z "${ASP_BIN:-}" ]; then \
        if [ -x target/release/asp ] && [ target/release/asp -nt "${protocol_bin}" ]; then \
          stale_reason="target/release/asp is newer"; \
        elif find crates/agent-semantic-protocol/src crates/agent-semantic-protocol/Cargo.toml Cargo.lock -type f -newer "${protocol_bin}" 2>/dev/null | grep -q .; then \
          stale_reason="agent-semantic-protocol Rust source is newer"; \
        fi; \
      fi; \
      if [ -n "${stale_reason}" ]; then \
        echo "[agent-tools-run-asp] stale ${protocol_bin}: ${stale_reason}; run \`just agent-tools-install-protocol ${bin_dir}\`" >&2; \
        exit 1; \
      fi; \
      SEMANTIC_AGENT_BIN_DIR="${bin_dir}" "${protocol_bin}" {{args}}; \
    else \
      SEMANTIC_AGENT_BIN_DIR="${bin_dir}" cargo run -q -p agent-semantic-protocol --bin asp -- {{args}}; \
    fi

# Install asp, asp-graph-turbo, and provider harnesses into $HOME/.local/bin by default, then install Codex hooks.
install bin_dir="":
	@just agent-tools-ensure-local-bin-path
	@just agent-hooks-install "{{bin_dir}}"

# Ensure interactive bash sessions can find the canonical local asp install.
agent-tools-ensure-local-bin-path:
	@marker="# agent-semantic-protocols: add local agent tools"; \
	  path_line='export PATH="$HOME/.local/bin:$PATH"'; \
	  touch "$HOME/.bashrc"; \
	  if ! grep -Fq '$HOME/.local/bin' "$HOME/.bashrc" && ! grep -Fq "$HOME/.local/bin" "$HOME/.bashrc"; then \
	    printf '\n%s\n%s\n' "${marker}" "${path_line}" >> "$HOME/.bashrc"; \
	    echo "[agent-tools-ensure-local-bin-path] added $HOME/.local/bin to ~/.bashrc"; \
	  else \
	    echo "[agent-tools-ensure-local-bin-path] ~/.bashrc already contains $HOME/.local/bin"; \
	  fi

# Install all agent tools, including asp-graph-turbo, and Codex hook config. Optional: just agent-hooks-install ~/.local/bin
agent-hooks-install bin_dir="":
	@bin_dir="{{bin_dir}}"; \
	if [ -z "${bin_dir}" ]; then bin_dir="${SEMANTIC_AGENT_BIN_DIR:-$HOME/.local/bin}"; fi; \
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
    @activation="$(cargo run -q -p agent-semantic-protocol --bin asp -- hook paths . | awk -F= '$1=="activation"{print substr($0, 12)}')"; \
      printf '%s' '{"tool_name":"functions.exec_command","tool_input":{"cmd":"sed -n '\''1,8p'\'' languages/typescript-lang-project-harness/tests/unit/cli.test.ts"}}' \
      | cargo run -q -p agent-semantic-protocol --bin asp -- hook pre-tool --client codex --activation "$activation" --config .codex/agent-semantic-protocol/hooks/config.toml --emit decision \
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

# Install asp, asp-graph-turbo, and all language provider harnesses under $HOME/.local/bin by default.
agent-tools-install-global bin_dir="":
    @bin_dir="{{bin_dir}}"; \
      if [ -z "${bin_dir}" ]; then bin_dir="${SEMANTIC_AGENT_BIN_DIR:-$HOME/.local/bin}"; fi; \
      just agent-tools-install-protocol "${bin_dir}"; \
      just agent-tools-install-asp-graph-turbo "${bin_dir}"; \
      just agent-tools-install-languages "${bin_dir}"; \
      echo "[agent-tools-install-global] installed asp, asp-graph-turbo, and all language provider harnesses into ${bin_dir}"

# Install all language provider harnesses under $HOME/.local/bin by default.
agent-tools-install-languages bin_dir="":
    @bin_dir="{{bin_dir}}"; \
      if [ -z "${bin_dir}" ]; then bin_dir="${SEMANTIC_AGENT_BIN_DIR:-$HOME/.local/bin}"; fi; \
      just agent-tools-install-rs "${bin_dir}"; \
      just agent-tools-install-ts "${bin_dir}"; \
      just agent-tools-install-py "${bin_dir}"; \
      just agent-tools-install-julia "${bin_dir}"; \
      just agent-tools-install-gerbil "${bin_dir}"; \
      echo "[agent-tools-install-languages] installed rs-harness, ts-harness, py-harness, asp-julia-harness, and gslph into ${bin_dir}"

# Install only the shared asp binary.
agent-tools-install-asp bin_dir="":
	@just agent-tools-install-protocol "{{bin_dir}}"

agent-tools-install-protocol bin_dir="":
    @requested_bin_dir="{{bin_dir}}"; \
      if [ -n "${requested_bin_dir}" ]; then \
        mkdir -p "${requested_bin_dir}"; \
        requested_bin_dir="$(cd "${requested_bin_dir}" && pwd -P)"; \
      fi; \
      cargo build --release --manifest-path Cargo.toml --package agent-semantic-protocol --bin asp || exit $?; \
      target/release/asp --version --require-release >/dev/null; \
      artifact_digest="$(shasum -a 256 target/release/asp | awk '{print $1}')"; \
      if [ -n "${requested_bin_dir}" ]; then \
        destinations="${requested_bin_dir}/asp"; \
      elif [ -n "${SEMANTIC_AGENT_BIN_DIR:-}" ]; then \
        destinations="${SEMANTIC_AGENT_BIN_DIR}/asp"; \
      else \
        destinations="$(printf '%s\n' "$HOME/.local/bin/asp" $(which -a asp 2>/dev/null || true) | awk 'NF && !seen[$0]++')"; \
      fi; \
      printf '%s\n' "${destinations}" | while IFS= read -r destination; do \
        [ -n "${destination}" ] || continue; \
        bin_dir="$(dirname "${destination}")"; \
        artifact_dir="${bin_dir}/.asp-versions/${artifact_digest}"; \
        mkdir -p "${artifact_dir}"; \
        install -m 755 target/release/asp "${artifact_dir}/asp"; \
        next_link="${bin_dir}/.asp-next.$$"; \
        ln -sfn "${artifact_dir}/asp" "${next_link}"; \
        mv -f "${next_link}" "${destination}"; \
        rm -f "${bin_dir}/semantic-agent-protocol"; \
        test -x "${destination}"; \
        "${destination}" --version --require-release >/dev/null; \
        "${destination}" guide >/dev/null; \
        printf '%s\n' "[agent-tools-install] binaryPath=${destination} binaryArtifactDigest=${artifact_digest} binarySwitch=atomic"; \
      done

# Install the debug protocol binary into a local bin dir and prewarm it.
agent-tools-install-protocol-debug bin_dir=".bin":
    @bin_dir="{{bin_dir}}"; \
      mkdir -p "${bin_dir}"; \
      cargo build --manifest-path Cargo.toml --package agent-semantic-protocol --bin asp; \
      install -m 755 target/debug/asp "${bin_dir}/asp"; \
      rm -f "${bin_dir}/semantic-agent-protocol"; \
      test -x "${bin_dir}/asp"; \
      "${bin_dir}/asp" guide >/dev/null

# Install the shared protocol binary used by hook runtime commands.
agent-tools-install-hook bin_dir="":
	@just agent-tools-install-protocol "{{bin_dir}}"

# Install a released language provider binary through asp.
# Target priority is owned by asp itself: asp.toml [languages.<id>].bin, SEMANTIC_AGENT_BIN_DIR, $HOME/.local/bin, PATH.
agent-tools-install-language language bin_dir="" target="" project=".":
	@args=(install language "{{language}}" --project "{{project}}"); \
	if [ -n "{{target}}" ]; then args+=(--target "{{target}}"); fi; \
	just _agent-tools-run-asp "{{bin_dir}}" "${args[@]}"

# Install only the core asp-graph-turbo ranking binary.
# Keep this entry repo-owned; uv tool install moves the same tool executable between bin dirs.
agent-tools-install-asp-graph-turbo bin_dir="":
    @bin_dir="{{bin_dir}}"; \
      if [ -z "${bin_dir}" ]; then bin_dir="${SEMANTIC_AGENT_BIN_DIR:-$HOME/.local/bin}"; fi; \
      mkdir -p "${bin_dir}"; \
      rm -f "${bin_dir}/asp-graph-turbo"; \
      printf '#!/usr/bin/env bash\nexec uv run --project "%s/packages/python/asp_graph_turbo" asp-graph-turbo "$@"\n' "$PWD" > "${bin_dir}/asp-graph-turbo"; \
      chmod 755 "${bin_dir}/asp-graph-turbo"; \
      rm -f "${bin_dir}/graph-turbo"; \
      test -x "${bin_dir}/asp-graph-turbo"; \
      "${bin_dir}/asp-graph-turbo" --help >/dev/null

# Install only the Rust provider binary.
agent-tools-install-rust bin_dir="":
    @just agent-tools-install-rs "{{bin_dir}}"

agent-tools-install-rs bin_dir="":
    @just agent-tools-install-language rust "{{bin_dir}}"

# Install only the TypeScript provider binary.
agent-tools-install-typescript bin_dir="":
    @just agent-tools-install-ts "{{bin_dir}}"

agent-tools-install-ts bin_dir="":
    @just agent-tools-install-language typescript "{{bin_dir}}"

# Install only the Python provider binary.
agent-tools-install-python bin_dir="":
    @just agent-tools-install-py "{{bin_dir}}"

agent-tools-install-py bin_dir="":
    @just agent-tools-install-language python "{{bin_dir}}"

# Install only the Julia provider binary.
agent-tools-install-julia bin_dir="":
    @just agent-tools-install-jl "{{bin_dir}}"

agent-tools-install-jl bin_dir="":
    @package_dir="$PWD/{{julia_harness_project}}"; \
      direnv exec . env \
        ASP_JULIA_BUILD_DIR="${package_dir}/build/juliac-asp-local" \
        ASP_JULIA_ALLOW_WRAPPER_FALLBACK=0 \
        "${package_dir}/juliac/build_provider.sh"; \
      direnv exec . just _agent-tools-run-asp "{{bin_dir}}" install language julia --from-workspace --project .

# Install only the Gerbil Scheme standalone binary.
agent-tools-install-gerbil bin_dir="":
    @just agent-tools-build-gerbil "{{bin_dir}}"

agent-tools-build-gerbil bin_dir="":
    @set -e; \
      repo_root="$PWD"; \
      bin_dir="{{bin_dir}}"; \
      if [ -z "${bin_dir}" ]; then bin_dir="${SEMANTIC_AGENT_BIN_DIR:-$HOME/.local/bin}"; fi; \
      package_dir="${repo_root}/{{gerbil_harness_project}}"; \
      root_bin="${repo_root}/.bin"; \
      cd "${package_dir}"; \
      if [ "$(uname -s)" = "Darwin" ]; then \
        env SDKROOT= CC="$(xcrun --find clang)" gxi \
          -e '(import :gslph/src/build-api/native-build)' \
          -e '(install-target #f #f #f #f #f #t)'; \
      else \
        gxi -e '(import :gslph/src/build-api/native-build)' \
          -e '(install-target #f #f #f #f #f #t)'; \
      fi; \
      launcher="$HOME/.local/bin/gslph"; \
      test -x "${launcher}"; \
      mkdir -p "${root_bin}" "${bin_dir}"; \
      if [ ! "${launcher}" -ef "${root_bin}/gslph" ]; then install -m 755 "${launcher}" "${root_bin}/gslph"; fi; \
      if [ ! "${launcher}" -ef "${bin_dir}/gslph" ]; then install -m 755 "${launcher}" "${bin_dir}/gslph"; fi; \
      test -x "${root_bin}/gslph"; \
      test -x "${bin_dir}/gslph"; \
      "${bin_dir}/gslph" --help >/dev/null

agent-tools-install-gx bin_dir="":
    @just agent-tools-build-gerbil "{{bin_dir}}"

agent-hooks-doctor-providers: agent-hooks-doctor-rs agent-hooks-doctor-ts agent-hooks-doctor-py agent-hooks-doctor-julia

agent-hooks-doctor-rs:
    rs-harness agent doctor {{repo}}

agent-hooks-doctor-ts:
    ts-harness agent doctor {{repo}}

agent-hooks-doctor-py:
    py-harness agent doctor {{repo}}

agent-hooks-doctor-julia:
    asp-julia-harness agent doctor --json {{julia_harness_project}} >/dev/null

check-sandtables:
    uv run --project packages/python python -m tools sandtable

benchmark-large-library-search-runtime:
    direnv exec . env ASP_BENCHMARK_BIN="$PWD/target/release/asp" uv run --project packages/python --frozen python -m tools.semantic_sandtable --repo-root . --large-library-runtime-benchmark --large-library-runtime-asp-bin target/release/asp --large-library-runtime-corpus-root .data

benchmark-large-library-search-runtime-baseline:
    receipt="$PWD/.cache/large-library-runtime-search.v1.receipt.json"; mkdir -p "$(dirname "$receipt")"; set +e; direnv exec . env ASP_BENCHMARK_BIN="$PWD/target/release/asp" uv run --project packages/python --frozen python -m tools.semantic_sandtable --repo-root . --json --large-library-runtime-benchmark --large-library-runtime-asp-bin target/release/asp --large-library-runtime-corpus-root .data > "$receipt"; runtime_status=$?; set -e; direnv exec . env ASP_BENCHMARK_BIN="$PWD/target/release/asp" uv run --project packages/python --frozen python -m tools.semantic_sandtable.large_library_runtime_baseline --baseline benchmarks/large-library-runtime-search.v1.baseline.json --receipt "$receipt"; baseline_status=$?; test "$runtime_status" -eq 0; test "$baseline_status" -eq 0

check-graph-turbo-focused:
    uv run --project packages/python/asp_graph_turbo --frozen pytest \
      tests/unit/test_asp_graph_turbo_request.py \
      tests/unit/test_asp_graph_turbo_feedback.py \
      tests/unit/test_asp_graph_turbo_calibration.py \
      tests/unit/test_asp_graph_turbo_projection_fields.py \
      tests/unit/test_asp_graph_turbo_ranking_collection_fields.py \
      tests/unit/test_asp_graph_turbo_read_loop.py \
      tests/unit/test_asp_graph_turbo_timeline.py \
      tests/unit/test_asp_graph_turbo_timeline_text.py \
      tests/unit/semantic_sandtable/test_agent_observation_pipe.py \
      tests/unit/semantic_sandtable/test_agent_observation_read_loop.py \
      tests/unit/semantic_sandtable/test_expectations.py

check-language-evidence-smoke-setup:
    mkdir -p .bin
    just agent-tools-install-protocol .bin
    just agent-tools-install-rs .bin
    just agent-tools-install-ts .bin
    just agent-tools-install-py .bin
    PATH="$PWD/.bin:$PATH" .bin/asp hook install --client codex .

check-language-evidence-smoke-core: check-language-evidence-smoke-setup
    protocol_home="$(PATH="$PWD/.bin:$PATH" .bin/asp hook paths . | awk -F= '$1=="protocolHome"{print substr($0, 14)}')" && \
      PATH="$PWD/.bin:$PATH" \
      ASP_LANGUAGE_EVIDENCE_SMOKE_SCOPE=core-fast \
      ASP_LANGUAGE_EVIDENCE_LANGUAGES=rust,python,typescript \
      ASP_LANGUAGE_EVIDENCE_TIMING_JSON="$protocol_home/language-evidence-smoke-core-fast.json" \
      uv run --project packages/python/asp_graph_turbo --frozen pytest tests/unit/test_language_evidence_smoke.py -q
    protocol_home="$(PATH="$PWD/.bin:$PATH" .bin/asp hook paths . | awk -F= '$1=="protocolHome"{print substr($0, 14)}')" && \
      cat "$protocol_home/language-evidence-smoke-core-fast.json"

check-language-evidence-smoke: check-language-evidence-smoke-core
    @true

check-provider-knowledge-axes:
    node tools/provider-knowledge-axes-close-loop.mjs

check-language-evidence-smoke-all-setup: check-language-evidence-smoke-setup
    just agent-tools-install-julia .bin
    PATH="$PWD/.bin:$PATH" .bin/asp hook install --client codex .
    PATH="$PWD/.bin:$PATH" .bin/asp julia guide {{julia_harness_project}} >/dev/null

check-language-evidence-smoke-all: check-language-evidence-smoke-all-setup
    protocol_home="$(PATH="$PWD/.bin:$PATH" .bin/asp hook paths . | awk -F= '$1=="protocolHome"{print substr($0, 14)}')" && \
      PATH="$PWD/.bin:$PATH" \
      ASP_LANGUAGE_EVIDENCE_SMOKE_SCOPE=all-providers \
      ASP_LANGUAGE_EVIDENCE_MAX_COMMAND_SECONDS_JULIA=2 \
      ASP_LANGUAGE_EVIDENCE_TIMING_JSON="$protocol_home/language-evidence-smoke-all-providers.json" \
      uv run --project packages/python/asp_graph_turbo --frozen pytest tests/unit/test_language_evidence_smoke.py -q
    protocol_home="$(PATH="$PWD/.bin:$PATH" .bin/asp hook paths . | awk -F= '$1=="protocolHome"{print substr($0, 14)}')" && \
      cat "$protocol_home/language-evidence-smoke-all-providers.json"

provider-gate: check-rust-warnings check-schema-profiles check-rfc-docs check-tree-sitter-query-contracts check-language-workspace-search-contracts check-graph-turbo-focused provider-gate-root provider-gate-rust provider-gate-typescript provider-gate-python provider-gate-julia

check-rust-warnings:
    env RUSTFLAGS="-D warnings" cargo check -q -p agent-semantic-protocol
    env RUSTFLAGS="-D warnings" cargo check -q --manifest-path {{rust_harness_project}}/Cargo.toml --features cli,search

check-schema-profiles:
    uv run --project packages/python python -m tools schema profiles validate

check-tree-sitter-query-contracts:
    uv run --project packages/python --frozen python -m tools tree-sitter validate contracts

check-language-workspace-search-contracts:
    uv run --project packages/python --frozen python -m tools validate language-workspace-search-contract

check-rfc-docs:
    uv run --project packages/python --frozen pytest \
      tests/unit/test_*rfc.py \
      tests/unit/test_docs_rfc_skill_contracts.py \
      -q

provider-gate-root: check-language-evidence-smoke
    just check-gerbil-owner-items-fast-path
    cargo test -p agent-semantic-hook
    uv run --project packages/python --frozen python -m pytest \
      tests/unit/test_semantic_*_schema.py \
      tests/unit/semantic_tree_sitter_query_rfc \
      tests/unit/test_cli_first_harness_ux_rfc.py \
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
        "{{gerbil_harness_project}}",
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
            "[gerbil-owner-items-fast] median latency exceeded threshold; "
            "keep `asp gerbil-scheme search owner ... items` on the Rust inline path"
        )

provider-gate-rust:
    cargo test --manifest-path {{rust_harness_project}}/Cargo.toml --features cli,search search
    cargo test --manifest-path {{rust_harness_project}}/Cargo.toml --features cli,search query
    cargo test --manifest-path {{rust_harness_project}}/Cargo.toml --features cli,search policy

provider-gate-typescript:
    npm --prefix {{typescript_harness_project}} run build
    npm --prefix {{typescript_harness_project}} run check:implementation
    node --test \
      {{typescript_harness_project}}/dist/tests/unit/cli_compact_query_snapshot.test.js \
      {{typescript_harness_project}}/dist/tests/unit/cli_ast_patch.test.js \
      {{typescript_harness_project}}/dist/tests/unit/cli_item_query.test.js \
      {{typescript_harness_project}}/dist/tests/unit/cli_item_query_code.test.js \
      {{typescript_harness_project}}/dist/tests/unit/cli_item_query_fallback.test.js \
      {{typescript_harness_project}}/dist/tests/unit/cli_search_ingest.test.js \
      {{typescript_harness_project}}/dist/tests/unit/cli_search_policy.test.js \
      {{typescript_harness_project}}/dist/tests/unit/cli_search_query.test.js \
      {{typescript_harness_project}}/dist/tests/unit/semantic_language_registry_read_packet.test.js \
      {{typescript_harness_project}}/dist/tests/unit/semantic_search_registry_expectations.js \
      {{typescript_harness_project}}/dist/tests/unit/semantic_search_schema.test.js

provider-gate-python:
    uv run --project {{python_harness_project}} --frozen py-harness search policy PY-PROJ-R001 owner tests --workspace {{python_harness_project}} --view seeds
    uv run --project {{python_harness_project}} --frozen py-harness search policy PY-AGENT-R008 owner tests --workspace {{python_harness_project}} --view seeds
    uv run --project {{python_harness_project}} --frozen py-harness query src/python_lang_project_harness/_semantic_language.py --term semantic_language_registry_document --names-only --workspace {{python_harness_project}}
    uv run --project {{python_harness_project}} --frozen python -m pytest \
      {{python_harness_project}}/tests/unit/harness/test_semantic_cli_query_set.py \
      {{python_harness_project}}/tests/unit/harness/test_semantic_cli_owner_items.py \
      {{python_harness_project}}/tests/unit/harness/test_semantic_search_ingest_cli.py \
      {{python_harness_project}}/tests/unit/harness/test_semantic_cli_policy.py \
      {{python_harness_project}}/tests/unit/harness/test_semantic_schema_registry.py

provider-gate-julia:
	julia --project={{julia_harness_project}} -e 'using Pkg; Pkg.test()'
	{{julia_harness}} guide {{julia_harness_project}} >/dev/null
	{{julia_harness}} agent doctor --json {{julia_harness_project}} >/dev/null
	{{julia_compiled_harness}} guide {{julia_harness_project}} >/dev/null
	{{julia_compiled_harness}} agent doctor --json {{julia_harness_project}} >/dev/null
	just check-language-evidence-smoke-all

# Refresh the local runtime boundary used by semantic-facts pipe smokes.
provider-gate-semantic-facts-setup:
    just agent-tools-install-py .bin
    just agent-hooks-install-current .bin

# Verify cross-language parser-owned data-shape facts through both provider ABI and asp pipe projection.
provider-gate-semantic-facts:
    #!/usr/bin/env python3
    import json
    import os
    import subprocess
    import sys
    from pathlib import Path

    root = Path.cwd()

    def run(argv: list[str], stdin: str | None = None) -> str:
        env = os.environ.copy()
        env["PATH"] = f"{root / '.bin'}:{env.get('PATH', '')}"
        proc = subprocess.run(
            argv,
            cwd=root,
            env=env,
            text=True,
            input=stdin,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if proc.returncode != 0:
            sys.stderr.write(f"[semantic-facts] command failed: {' '.join(argv)}\n")
            sys.stderr.write(proc.stderr)
            sys.stderr.write(proc.stdout)
            raise SystemExit(proc.returncode)
        return proc.stdout

    def require(condition: bool, message: str) -> None:
        if not condition:
            raise SystemExit(f"[semantic-facts] {message}")

    required_bins = [
        root / ".bin" / "asp",
        root / ".bin" / "rs-harness",
        root / ".bin" / "ts-harness",
        root / ".bin" / "py-harness",
        root / "{{julia_compiled_harness}}",
    ]
    missing = [str(path) for path in required_bins if not os.access(path, os.X_OK)]
    require(
        not missing,
        "missing local runtime executable(s): "
        + ", ".join(missing)
        + "; run `just provider-gate-semantic-facts-setup` from an environment-loaded shell",
    )

    doctor = run([str(root / ".bin" / "asp"), "hook", "doctor", "--client", "codex", "."])
    doctor_lines = doctor.splitlines()
    for language in ("rust", "typescript", "python", "julia"):
        provider_line = next((line for line in doctor_lines if f"language={language} " in line), "")
        require(
            provider_line and "runtimeStatus=available" in provider_line,
            f"hook doctor did not report available runtime for {language}",
        )

    direct_cases = [
        (
            "rust",
            [str(root / ".bin" / "rs-harness"), "search", "semantic-facts", "Vec collection fields", "--json", "{{rust_harness_project}}"],
            "src/cli/dev_command_log/command.rs:9:1:pipes: Vec<String>\n",
        ),
        (
            "typescript",
            [str(root / ".bin" / "ts-harness"), "search", "semantic-facts", "array collection fields", "--json", "{{typescript_harness_project}}"],
            "src/cli/dev-command-log.ts:70:1:pipes: readonly string[]\n",
        ),
        (
            "python",
            [str(root / ".bin" / "py-harness"), "search", "semantic-facts", "list collection fields", "--json", "{{python_harness_project}}"],
            "src/python_lang_parser/_ast_collector.py:42:1:_scope_stack: list[str]\n",
        ),
        (
            "julia",
            [str(root / "{{julia_compiled_harness}}"), "search", "semantic-facts", "Vector collection fields", "--json", "{{julia_harness_project}}"],
            "src/cli.jl:14:1:tags::Vector{String}\n",
        ),
    ]

    def node_kind(node: dict) -> str:
        value = node.get("kind")
        if isinstance(value, str) and value:
            return value
        node_id = node.get("id")
        if isinstance(node_id, str) and ":" in node_id:
            return node_id.split(":", 1)[0]
        return ""

    def edge_relation(edge: dict) -> str:
        for key in ("relation", "rel", "label", "kind"):
            value = edge.get(key)
            if isinstance(value, str) and value:
                return value
        return ""

    for language, argv, stdin in direct_cases:
        packet = json.loads(run(argv, stdin=stdin))
        nodes = packet.get("nodes", [])
        edges = packet.get("edges", [])
        kinds = {node_kind(node) for node in nodes if isinstance(node, dict)}
        relations = {edge_relation(edge) for edge in edges if isinstance(edge, dict)}
        require(nodes, f"{language} semantic-facts returned no nodes")
        require(edges, f"{language} semantic-facts returned no edges")
        require("field" in kinds, f"{language} semantic-facts missing field node")
        require("type" in kinds, f"{language} semantic-facts missing type node")
        require("collection" in kinds, f"{language} semantic-facts missing collection node")
        require("collection_of" in relations, f"{language} semantic-facts missing collection_of edge")
        print(f"[semantic-facts] direct {language} nodes={len(nodes)} edges={len(edges)}")

    pipe_cases = [
        ("rust", "Vec collection fields", "{{rust_harness_project}}"),
        ("typescript", "array collection fields", "{{typescript_harness_project}}"),
        ("python", "list collection fields", "{{python_harness_project}}"),
        ("julia", "Vector collection fields", "{{julia_harness_project}}"),
    ]

    for language, query, project in pipe_cases:
        output = run([str(root / ".bin" / "asp"), language, "search", "pipe", query, "--view", "seeds", project])
        require("[graph-frontier]" in output, f"{language} pipe missing graph frontier")
        require("field:" in output, f"{language} pipe missing compact field node")
        require("collection:family(" in output, f"{language} pipe missing compact collection node")
        require("recommendedNext=S1.query-selector" in output, f"{language} pipe missing selector-first recommendation")
        require(f"nextCommand=asp {language} query --selector" in output, f"{language} pipe missing next query command")
        require(
            f"--workspace {project} --code" in output,
            f"{language} pipe missing scoped --workspace root {project}",
        )
        print(f"[semantic-facts] pipe {language} ok")

perf-calibrate-julia-cache:
	cargo build -q -p agent-semantic-protocol --bin asp
	@tmp="$(mktemp -d)"; \
	  asp_bin="$PWD/target/debug/asp"; \
	  "${asp_bin}" cache invalidate --root {{julia_harness_project}} >/dev/null; \
	  "${asp_bin}" julia search prime --view seeds {{julia_harness_project}} --receipt-json >"${tmp}/miss.out" 2>"${tmp}/miss.receipt.json"; \
	  "${asp_bin}" julia search prime --view seeds {{julia_harness_project}} --receipt-json >"${tmp}/hit.out" 2>"${tmp}/hit.receipt.json"; \
	  uv run --project packages/python --frozen python -m tools cache validate julia-performance "${tmp}"

check-python-policy:
    uv run --project {{python_harness_project}} --frozen py-harness check --full {{repo}}

report-python-policy:
    uv run --project {{python_harness_project}} --frozen py-harness check --full {{repo}} || true

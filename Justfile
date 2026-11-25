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

# Sync external language providers/analyzers declared in providers.toml (replaces .gitmodules).
providers-sync profile="core":
	python3 scripts/providers_sync.py sync --workspace . --profile "{{profile}}"

# Show sync status of external providers/analyzers declared in providers.toml.
providers-status profile="all":
	python3 scripts/providers_sync.py status --workspace . --profile "{{profile}}"

# Lock current checked-out HEAD commits into providers.toml.
providers-lock profile="core":
	python3 scripts/providers_sync.py lock --workspace . --profile "{{profile}}"

# Symlink local ghq checkouts into languages/ and analyzers/ for multi-repo co-development.
providers-link-local ghq_root="$HOME/ghq" profile="core":
	python3 scripts/providers_sync.py link-local --workspace . --profile "{{profile}}" --ghq-root "{{ghq_root}}"

agent-tools-build-gerbil bin_dir="":
    @set -e; \
      repo_root="$PWD"; \
      package_dir="${repo_root}/{{asp_gerbil_scheme_project}}"; \
      artifact_root="${package_dir}/build/workspace-provider"; \
      cd "${package_dir}"; \
      gxpkg env gxi ./build-provider.ss compile --release; \
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
      artifact_root="${package_dir}/build/workspace-provider"; \
      cd "${package_dir}"; \
      GERBIL_PATH="${package_dir}/.gerbil" \
        GERBIL_LOADPATH="${artifact_root}/lib:${package_dir}/.gerbil/lib" \
        gxtest t/projection-batch-test.ss; \
      GERBIL_PATH="${package_dir}/.gerbil" \
        GERBIL_LOADPATH="${artifact_root}/lib:${package_dir}/.gerbil/lib" \
        gxtest t/provider-http-json-server-test.ss

agent-tools-install-gx bin_dir="":
    @just agent-tools-build-gerbil "{{bin_dir}}"

agent-tools-doctor-providers: agent-tools-doctor-rs agent-tools-doctor-ts agent-tools-doctor-py agent-tools-doctor-julia

agent-tools-doctor-rs:
    asp-rust agent doctor {{repo}}

agent-tools-doctor-ts:
    asp-typescript agent doctor {{repo}}

agent-tools-doctor-py:
    asp-python agent doctor {{repo}}

agent-tools-doctor-julia:
    asp-julia agent doctor --json {{asp_julia_project}} >/dev/null

check-sandtables:
    uv run --project packages/python python -m tools sandtable

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

provider-gate: check-rust-warnings check-schema-profiles check-rfc-docs check-rust-workspace-policy check-schema-manager check-graph-turbo-focused provider-gate-root provider-gate-rust provider-gate-typescript provider-gate-python provider-gate-julia provider-gate-gerbil

# Run the parser-owned whole-workspace policy exactly once. Ordinary member
# builds keep the O(1) manifest/policy-identity dependency and never rescan the
# crate source tree.
check-rust-workspace-policy:
    rtk cargo test -p asp-rust-project-harness-policy --features workspace-policy --test integration_test workspace_policy::asp_workspace_source_policy_is_clean -- --exact --nocapture

check-rust-warnings:
    env RUSTFLAGS="-D warnings" cargo check -q --workspace
    env RUSTFLAGS="-D warnings" cargo check -q --manifest-path {{asp_rust_project}}/Cargo.toml --all-features

check-schema-profiles:
    rtk cargo test -p agent-semantic-schema-manager canonical_registry_verifies_every_registered_language_bundle --quiet

check-schema-manager: check-schema-proof-plan
	uv run --project packages/python --frozen --exact asp-schema-manager check --workspace-root . --fail-on-family-local-refs --fail-on-unclassified-schemas --fail-on-mixed-family-refs --fail-on-reference-decision-drift

check-schema-proof-plan:
    uv run --project packages/python --frozen --exact pytest packages/python/asp_schema_manager/tests/unit/test_logical_projection.py packages/python/asp_schema_manager/tests/unit/test_proof_plan_cli.py -q

report-schema-manager:
    uv run --project packages/python --frozen --exact asp-schema-manager audit --workspace-root .

check-rfc-docs:
    uv run --project packages/python --frozen --exact pytest \
      tests/unit/test_*rfc.py \
      tests/unit/test_docs_rfc_skill_contracts.py \
      -q

provider-gate-root: check-language-facade-smoke
    just check-gerbil-owner-items-fast-path
    uv run --project packages/python --frozen --exact python -m pytest \
      tests/unit/test_semantic_*_schema.py \
      tests/unit/test_asp_server_first_architecture_rfc.py \
      tests/unit/test_marlin_host_integration_rfc.py \
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
            "run `just agent-tools-install-client`"
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
      {{asp_python_project}}/tests/unit/asp_python/test_semantic_cli.py \
      {{asp_python_project}}/tests/unit/asp_python/test_semantic_language_schemas.py

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

# Validate OpenSysML v2 models under project.sysml and dev/sysml.
sysml-check:
    sysml -strict -validate -self-check -no-record-cache project.sysml dev/sysml

# Verify Quint search engine state-machine models and negative mutation controls.
quint-check:
    quint typecheck dev/quint/search-engine.qnt
    quint test dev/quint/search-engine.qnt --main=searchGenerationPublication
    quint test dev/quint/search-engine.qnt --main=searchPlanAbiAdmission
    quint test dev/quint/search-engine.qnt --main=searchTopologyEdgeCustody
    quint run dev/quint/search-engine.qnt --main=searchGenerationPublication --seed=42 --max-samples=1000 --max-steps=16 --invariant=safe --verbosity=1
    quint run dev/quint/search-engine.qnt --main=searchPlanAbiAdmission --seed=42 --max-samples=1000 --max-steps=16 --invariant=safe --verbosity=1
    quint run dev/quint/search-engine.qnt --main=searchTopologyEdgeCustody --seed=42 --max-samples=1000 --max-steps=16 --invariant=safe --verbosity=1
    @for control in searchGenerationPublication:publishWithoutBlobMutation searchGenerationPublication:bypassCasMutation searchGenerationPublication:nonMonotonicRevisionMutation searchPlanAbiAdmission:bypassAdmissionMutation searchPlanAbiAdmission:staleGenerationMutation searchTopologyEdgeCustody:crossGenerationLeakMutation searchTopologyEdgeCustody:duplicateCountMutation; do result=0; output=$(quint run dev/quint/search-engine.qnt --main="${control%:*}" --step="${control#*:}" --seed=42 --max-samples=1000 --max-steps=16 --invariant=safe --verbosity=1 2>&1) || result=$?; printf '%s\n%s\n' "$control" "$output"; if [ "$result" -ne 1 ] || ! printf '%s\n' "$output" | grep -q 'Invariant violated'; then exit 1; fi; done

# Combined formal architecture, Quint state model, and Rust stateful property gate.
property-gate: sysml-check quint-check
    cargo test --locked -p agent-semantic-search --test marlin_engine_state_machine

# Multi-scale Criterion hot-path benchmarks for the ASP search engine.
search-engine-bench:
    cargo bench --locked -p agent-semantic-search --bench search_engine_hot_path

report-python-policy:
    uv run --project {{asp_python_project}} --frozen python -c 'from asp_python import render_asp_python_report, run_asp_python; print(render_asp_python_report(run_asp_python("{{repo}}")), end="")'

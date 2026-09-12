# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import re
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
CI_WORKFLOW = REPO_ROOT / ".github" / "workflows" / "ci.yml"
JUSTFILE = REPO_ROOT / "Justfile"
RELEASE_WORKFLOW = REPO_ROOT / ".github" / "workflows" / "release.yml"


def test_root_schema_gate_references_only_present_test_paths() -> None:
    workflow = CI_WORKFLOW.read_text(encoding="utf-8")
    root_schema_gate = workflow.split("- name: Root schema gates", 1)[1]
    root_schema_gate = root_schema_gate.split("- name: Python provider schema gates", 1)[0]
    referenced_paths = sorted(set(re.findall(r"tests/[A-Za-z0-9_./-]+", root_schema_gate)))

    assert referenced_paths
    assert [path for path in referenced_paths if not (REPO_ROOT / path).exists()] == []


def test_asp_rust_ci_checks_out_provider_catalog_submodules() -> None:
    workflow = CI_WORKFLOW.read_text(encoding="utf-8")

    rust_checkout_step = workflow.split(
        "- name: Checkout provider catalog submodules", 1
    )[1]
    rust_checkout_step = rust_checkout_step.split("- name: Setup Rust", 1)[0]
    schema_checkout_step = workflow.split("- name: Checkout provider submodules", 1)[1]
    schema_checkout_step = schema_checkout_step.split("- name: Install uv", 1)[0]

    for checkout_step in (rust_checkout_step, schema_checkout_step):
        assert "languages/AspJulia.jl" in checkout_step
        assert "languages/asp-gerbil-scheme" in checkout_step


def test_root_release_carries_server_managed_graphs_artifact() -> None:
    workflow = RELEASE_WORKFLOW.read_text(encoding="utf-8")

    assert "Build atomic ASP and asp-python-graphs service bundle" in workflow
    assert "cargo build --release --manifest-path Cargo.toml --package agent-semantic-client --bin asp" in workflow
    assert "uv build --project packages/python/asp_python_graphs --wheel" in workflow
    assert "package/asp-python-graphs.bundle" in workflow
    assert '"artifactKind": "server-managed-service-environment"' in workflow
    assert '"project": "asp-python-graphs"' in workflow
    assert '"algorithm": "graph-turbo"' in workflow
    assert "python -m venv --copies" in workflow
    assert "asp-python-graphs-service" in workflow
    assert "asp_python_graphs.service_cli" in workflow
    assert "packages/python/asp_graph_turbo" not in workflow
    assert "graph_turbo_cli" not in workflow
    assert "graph artifact publish" not in workflow
    assert "asp-python-graphs-artifact.v2.json" not in workflow
    assert '"publicStandaloneCommand": False' in workflow
    assert "asp-graph-turbo" not in workflow
    assert "asp-graph-turbo-resident" not in workflow


def test_tree_sitter_contract_gate_uses_packaged_cli() -> None:
    workflow = CI_WORKFLOW.read_text(encoding="utf-8")

    assert "tools/run-tree-sitter-query-contracts.sh" not in workflow
    assert (
        "uv run --project packages/python --frozen python -m tools "
        "tree-sitter validate contracts"
    ) in workflow


def test_language_facade_ci_gate_is_static() -> None:
    workflow = CI_WORKFLOW.read_text(encoding="utf-8")

    step = workflow.split("- name: Language facade smoke gate", 1)[1]
    step = step.split("- name: Tree-sitter query contract gates", 1)[0]

    assert (
        "uv run --project packages/python/asp_python_graphs --frozen pytest "
        "tests/unit/test_language_facade_smoke.py -q"
    ) in step
    for obsolete_setup in (
        "ASP_LANGUAGE_FACADE_SCOPE",
        "ASP_LANGUAGE_FACADE_LANGUAGES",
        "npm install --global @openai/codex",
        "asp install plugin",
        "asp.toml.ci-full-provider",
        "cargo build",
        "install -m 755",
    ):
        assert obsolete_setup not in step


def test_live_corpus_setup_owns_provider_installation() -> None:
    justfile = JUSTFILE.read_text(encoding="utf-8")

    facade = justfile.split("check-language-facade-smoke:", 1)[1]
    facade = facade.split("check-provider-knowledge-axes:", 1)[0]
    assert "agent-tools-install" not in facade
    assert "ASP_LANGUAGE_FACADE" not in facade

    setup = justfile.split("check-live-corpus-search-query-all-setup:", 1)[1]
    setup = setup.split("check-live-corpus-search-query-all:", 1)[0]
    for provider in ("protocol", "rs", "ts", "py", "julia", "orgize"):
        assert f"just agent-tools-install-{provider}" in setup
    assert "agent-tools-install-protocol .bin" not in setup


def test_agent_tools_run_asp_rejects_stale_default_binary() -> None:
    justfile = JUSTFILE.read_text(encoding="utf-8")

    runner = justfile.split("_agent-tools-run-asp bin_dir +args:", 1)[1]
    runner = runner.split("# Install asp, asp-python-graphs", 1)[0]

    assert 'protocol_bin="${ASP_BIN:-${bin_dir}/asp}"' in runner
    assert '[ -z "${ASP_BIN:-}" ]' in runner
    assert '[ target/release/asp -nt "${protocol_bin}" ]' in runner
    assert "crates/agent-semantic-protocol/src" in runner
    assert "agent-semantic-protocol Rust source is newer" in runner
    assert "run \\`just agent-tools-install-protocol ${bin_dir}\\`" in runner


def test_gerbil_owner_items_fast_path_gate_uses_rust_inline_and_millisecond_budget() -> (
    None
):
    justfile = JUSTFILE.read_text(encoding="utf-8")

    provider_gate_root = justfile.split("provider-gate-root:", 1)[1]
    provider_gate_root = provider_gate_root.split("provider-gate-rust:", 1)[0]
    assert "just check-gerbil-owner-items-fast-path" in provider_gate_root

    gate = justfile.split("check-gerbil-owner-items-fast-path:", 1)[1]
    gate = gate.split("provider-gate-rust:", 1)[0]

    assert "ASP_GERBIL_OWNER_ITEMS_MAX_SECONDS" in gate
    assert '"0.25"' in gate
    assert "ASP_GERBIL_OWNER_ITEMS_RUNS" in gate
    assert "statistics.median" in gate
    assert "reason=rust-inline-gerbil-owner-items" in gate
    assert "source=rust-inline" in gate
    assert "no fallback to Gerbil provider is allowed" in gate
    assert "build.ss" in gate
    assert "{{asp_gerbil_scheme_project}}" in gate


def test_gerbil_just_build_uses_canonical_provider_entrypoint() -> None:
    justfile = JUSTFILE.read_text(encoding="utf-8")

    target = justfile.split('agent-tools-build-gerbil bin_dir="":', 1)[1]
    target = target.split('agent-tools-install-gx bin_dir="":', 1)[0]

    assert 'artifact_root="${package_dir}/build/workspace-provider"' in target
    assert "gxpkg env gxi ./build-provider.ss compile" in target
    assert "ASP_PROVIDER_ARTIFACT_ROOT" not in target
    assert 'provider_binary="${artifact_root}/bin/asp-gerbil-scheme"' in target
    assert 'cp "${provider_binary}" "{{bin_dir}}/asp-gerbil-scheme"' in target
    assert 'provider=asp-gerbil-scheme' in target


def test_julia_full_provider_gate_uses_fresh_compiled_provider_perf_guard() -> None:
    justfile = JUSTFILE.read_text(encoding="utf-8")

    install_julia = justfile.split('agent-tools-install-jl bin_dir="":', 1)[1]
    install_julia = install_julia.split("agent-hooks-doctor-providers:", 1)[0]
    assert "ASP_JULIA_ALLOW_WRAPPER_FALLBACK=0" in install_julia
    assert "just agent-tools-install-language julia" in install_julia
    assert 'provider_bin="$${state_home}/runtime/bin/asp-julia"' in install_julia

    live_corpus_setup = justfile.split(
        "check-live-corpus-search-query-all-setup:", 1
    )[1]
    live_corpus_setup = live_corpus_setup.split(
        "check-live-corpus-search-query-all:", 1
    )[0]
    assert "just agent-tools-install-julia" in live_corpus_setup
    assert ".bin/asp julia guide" not in live_corpus_setup

    provider_gate_julia = justfile.split("provider-gate-julia:", 1)[1]
    provider_gate_julia = provider_gate_julia.split(
        "provider-gate-semantic-facts-setup:", 1
    )[0]
    assert "just check-language-facade-smoke" in provider_gate_julia
    assert "check-language-facade-smoke-all" not in provider_gate_julia

# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Validate the artifact and ownership boundaries in the CI contract DAG."""

from pathlib import Path


CI_WORKFLOW = Path(__file__).resolve().parents[2] / ".github" / "workflows" / "ci.yml"


def test_contract_gates_form_a_parallel_dag_around_one_asp_binary() -> None:
    workflow = CI_WORKFLOW.read_text(encoding="utf-8")

    assert workflow.count("cargo build --bin asp") == 2
    contract_jobs = workflow.split("  asp-linux-binary:", 1)[1]
    assert contract_jobs.count("cargo build --bin asp") == 1
    assert "name: asp-linux-contract-binary" in contract_jobs
    assert "  shared-contract-gates:" in contract_jobs
    assert "  python-provider-gates:" in contract_jobs
    assert "  catalog-provider-gates:" in contract_jobs
    assert "  rust-provider-gates:" in contract_jobs
    assert "  tree-sitter-contract-gates:" in contract_jobs

    python_provider = contract_jobs.split("  python-provider-gates:", 1)[1].split(
        "  catalog-provider-gates:", 1
    )[0]
    assert "languages/asp-python" in python_provider
    assert "cargo test" not in python_provider
    assert "Set up Rust" not in python_provider

    catalog_provider = contract_jobs.split("  catalog-provider-gates:", 1)[1].split(
        "  rust-provider-gates:", 1
    )[0]
    assert (
        "canonical_client_profile_publishes_the_shared_schema_bundle_route"
        in catalog_provider
    )
    assert (
        "host_uds_schema_bundle_route_bypasses_workspace_generation"
        in catalog_provider
    )

    rust_provider = contract_jobs.split("  rust-provider-gates:", 1)[1].split(
        "  tree-sitter-contract-gates:", 1
    )[0]
    assert "needs: asp-linux-binary" in rust_provider
    assert "actions/download-artifact@v4" in rust_provider
    assert "cargo run --quiet --bin asp" not in rust_provider
    assert ".ci/bin/asp schema materialize" in rust_provider

    tree_sitter = contract_jobs.split("  tree-sitter-contract-gates:", 1)[1]
    assert "needs: asp-linux-binary" in tree_sitter
    assert "--asp-bin .ci/bin/asp" in tree_sitter
    assert "--no-build" in tree_sitter

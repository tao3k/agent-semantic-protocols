from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
CI_WORKFLOW = REPO_ROOT / ".github" / "workflows" / "ci.yml"
JUSTFILE = REPO_ROOT / "Justfile"

LANGUAGE_RELEASE_WORKFLOWS = {
    "languages/rust-lang-project-harness": {
        "binary": "rs-harness",
        "darwin_os": "macos-14",
        "targets": {
            "x86_64-unknown-linux-gnu",
            "aarch64-apple-darwin",
            "x86_64-pc-windows-msvc",
        },
    },
    "languages/typescript-lang-project-harness": {
        "binary": "ts-harness",
        "darwin_os": "ubuntu-latest",
        "targets": {
            "x86_64-unknown-linux-gnu",
            "aarch64-apple-darwin",
        },
    },
    "languages/python-lang-project-harness": {
        "binary": "py-harness",
        "darwin_os": "macos-latest",
        "targets": {
            "x86_64-unknown-linux-gnu",
            "aarch64-apple-darwin",
        },
    },
    "languages/JuliaLangProjectHarness.jl": {
        "binary": "asp-julia-harness",
        "darwin_os": "macos-14",
        "targets": {
            "x86_64-unknown-linux-gnu",
            "aarch64-apple-darwin",
        },
    },
    "languages/gerbil-scheme-language-project-harness": {
        "binary": "gslph",
        "darwin_os": "ubuntu-latest",
        "targets": {
            "x86_64-unknown-linux-gnu",
            "aarch64-apple-darwin",
        },
    },
    "languages/orgize": {
        "binary": "orgize",
        "darwin_os": "macos-14",
        "targets": {
            "x86_64-unknown-linux-gnu",
            "aarch64-apple-darwin",
            "x86_64-pc-windows-msvc",
        },
    },
}


def test_language_release_workflows_are_project_owned_and_publish_assets() -> None:
    for language_path, contract in LANGUAGE_RELEASE_WORKFLOWS.items():
        workflow_path = REPO_ROOT / language_path / ".github" / "workflows" / "release.yml"
        assert workflow_path.exists(), language_path

        workflow = workflow_path.read_text(encoding="utf-8")

        assert "name: Release provider binary" in workflow
        assert "workflow_dispatch:" in workflow
        assert "release:" in workflow
        assert "types:" in workflow
        assert "- published" in workflow
        assert "push:" in workflow
        assert "tags:" in workflow
        assert '- "v*"' in workflow
        assert "permissions:\n  contents: write" in workflow
        assert f"BINARY: {contract['binary']}" in workflow
        assert "github.event.release.tag_name || inputs.tag || github.ref_name" in workflow
        assert "- name: Ensure release tag" in workflow
        assert "if: github.event_name == 'workflow_dispatch'" in workflow
        assert "release tag must start with v" in workflow
        assert 'git push origin "refs/tags/${RELEASE_TAG}"' in workflow
        assert "gh release create" in workflow
        assert "gh release upload" in workflow
        assert "--clobber" in workflow
        assert ".sha256" in workflow
        assert "x86_64-apple-darwin" not in workflow

        if "x86_64-pc-windows-msvc" in contract["targets"]:
            assert "- name: Enable Windows long paths" in workflow
            assert "git config --global core.longpaths true" in workflow
            assert 'CARGO_NET_GIT_FETCH_WITH_CLI=true' in workflow
            build_step = workflow.split("- name: Build release binary", 1)[1]
            build_step = build_step.split("- name: Package provider binary", 1)[0]
            assert "shell: bash" in build_step

        for target in contract["targets"]:
            assert target in workflow, f"{language_path} missing {target}"

        assert (
            f"- os: {contract['darwin_os']}\n"
            "            target: aarch64-apple-darwin"
        ) in workflow

        if language_path == "languages/gerbil-scheme-language-project-harness":
            assert "- name: Build Gerbil" in workflow
            assert "gxpkg deps --install" in workflow
            assert "- name: Build native binary" in workflow
            assert "gxpkg env ./build.ss compile --release --optimized" in workflow
            assert ".bin/gslph search prime --view seeds --workspace ." in workflow
            assert ".bin/gslph search workspace-scope --workspace ." in workflow
            assert "package/bin/gslph" in workflow


def test_asp_rust_ci_checks_out_provider_catalog_submodules() -> None:
    workflow = CI_WORKFLOW.read_text(encoding="utf-8")

    rust_checkout_step = workflow.split("- name: Checkout provider catalog submodules", 1)[1]
    rust_checkout_step = rust_checkout_step.split("- name: Setup Rust", 1)[0]
    schema_checkout_step = workflow.split("- name: Checkout provider submodules", 1)[1]
    schema_checkout_step = schema_checkout_step.split("- name: Install uv", 1)[0]

    for checkout_step in (rust_checkout_step, schema_checkout_step):
        assert "languages/JuliaLangProjectHarness.jl" in checkout_step
        assert "languages/gerbil-scheme-language-project-harness" in checkout_step


def test_tree_sitter_contract_gate_uses_packaged_cli() -> None:
    workflow = CI_WORKFLOW.read_text(encoding="utf-8")

    assert 'tools/run-tree-sitter-query-contracts.sh' not in workflow
    assert (
        "uv run --project packages/python --frozen python -m tools "
        "tree-sitter validate contracts"
    ) in workflow


def test_language_evidence_ci_hot_path_stays_core_fast() -> None:
    workflow = CI_WORKFLOW.read_text(encoding="utf-8")

    step = workflow.split("- name: Language evidence and facade smoke gate", 1)[1]
    step = step.split("- name: Tree-sitter query contract gates", 1)[0]

    assert "ASP_LANGUAGE_EVIDENCE_SMOKE_SCOPE=core-fast" in step
    assert "ASP_LANGUAGE_EVIDENCE_LANGUAGES=rust,python,typescript" in step
    assert "language-evidence-smoke-core-fast.json" in step
    assert "asp install plugin --codex ." in step
    assert "asp.toml.ci-full-provider" in step
    assert "[providers.gerbil-scheme]" in step
    assert "[providers.julia]" in step
    assert "enabled = false" in step
    assert "asp-julia-harness" not in step
    assert ".bin/gerbil-scheme-harness" not in step
    assert "agent-tools-install-julia" not in step


def test_language_evidence_setup_installs_release_asp_binary() -> None:
    justfile = JUSTFILE.read_text(encoding="utf-8")

    setup = justfile.split("check-language-evidence-smoke-setup:", 1)[1]
    setup = setup.split("check-language-evidence-smoke-core:", 1)[0]

    assert "just agent-tools-install-protocol .bin" in setup
    assert "target/debug/asp" not in setup
    assert "cargo build -q --manifest-path Cargo.toml --package agent-semantic-protocol --bin asp" not in setup


def test_agent_tools_run_asp_rejects_stale_default_binary() -> None:
    justfile = JUSTFILE.read_text(encoding="utf-8")

    runner = justfile.split("_agent-tools-run-asp bin_dir +args:", 1)[1]
    runner = runner.split("# Install asp, asp-graph-turbo", 1)[0]

    assert 'protocol_bin="${ASP_BIN:-${bin_dir}/asp}"' in runner
    assert '[ -z "${ASP_BIN:-}" ]' in runner
    assert '[ target/release/asp -nt "${protocol_bin}" ]' in runner
    assert "crates/agent-semantic-protocol/src" in runner
    assert "agent-semantic-protocol Rust source is newer" in runner
    assert "run \\`just agent-tools-install-protocol ${bin_dir}\\`" in runner


def test_gerbil_owner_items_fast_path_gate_uses_rust_inline_and_millisecond_budget() -> None:
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
    assert "{{gerbil_harness_project}}" in gate


def test_gerbil_ci_uses_canonical_gslph_binary() -> None:
    workflow_path = (
        REPO_ROOT
        / "languages"
        / "gerbil-scheme-language-project-harness"
        / ".github"
        / "workflows"
        / "ci.yml"
    )
    workflow = workflow_path.read_text(encoding="utf-8")

    assert "- name: Build canonical gslph binary" in workflow
    assert "gxpkg env ./build.ss compile --release --optimized" in workflow
    assert "test -x .bin/gslph" in workflow
    assert "- name: Smoke canonical search subcommands" in workflow
    assert ".bin/gslph search prime --view seeds --workspace ." in workflow
    assert ".bin/gslph search workspace-scope --workspace ." in workflow
    assert ".bin/gslph check --full ." in workflow
    assert ".bin/gslph bench --json" in workflow
    assert ".bin/gslph search prime --json ." in workflow


def test_gerbil_just_build_scans_only_launcher_build_inputs() -> None:
    justfile = JUSTFILE.read_text(encoding="utf-8")

    target = justfile.split('agent-tools-build-gerbil bin_dir="":', 1)[1]
    target = target.split('agent-tools-install-gx bin_dir="":', 1)[0]

    assert 'launcher="${package_bin}/gslph"' in target
    assert (
        "find src/cli-launcher.ss src/cli-dev-linker.ss "
        "src/search-light-launcher.ss src/constants.ss "
        "src/commands/search-prime-light.ss build.ss gerbil.pkg version.ss"
    ) in target
    assert 'GERBIL_BUILD_CORES="${cores}" ./build.ss compile --binary --optimized' in target
    assert "find src build.ss gerbil.pkg version.ss" not in justfile
    assert "[agent-tools-build-gerbil] ${launcher} is up to date" in target


def test_julia_full_provider_gate_uses_fresh_compiled_harness_perf_guard() -> None:
    justfile = JUSTFILE.read_text(encoding="utf-8")

    install_julia = justfile.split('agent-tools-install-jl bin_dir="":', 1)[1]
    install_julia = install_julia.split("agent-hooks-doctor-providers:", 1)[0]
    assert "agent-tools-install-language julia" in install_julia
    assert "juliac/build_provider.sh" not in install_julia
    assert 'install -m 755 "{{julia_compiled_harness}}"' not in install_julia

    all_smoke = justfile.split("check-language-evidence-smoke-all-setup:", 1)[1]
    all_smoke = all_smoke.split("provider-gate:", 1)[0]
    assert "just agent-tools-install-julia .bin" in all_smoke
    assert ".bin/asp julia guide {{julia_harness_project}} >/dev/null" in all_smoke
    assert "ASP_LANGUAGE_EVIDENCE_SMOKE_SCOPE=all-providers" in all_smoke
    assert "ASP_LANGUAGE_EVIDENCE_MAX_COMMAND_SECONDS_JULIA=2" in all_smoke

    provider_gate_julia = justfile.split("provider-gate-julia:", 1)[1]
    provider_gate_julia = provider_gate_julia.split("provider-gate-semantic-facts-setup:", 1)[0]
    assert "just check-language-evidence-smoke-all" in provider_gate_julia

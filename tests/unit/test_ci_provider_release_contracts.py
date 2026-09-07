# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]

LANGUAGE_RELEASE_WORKFLOWS = {
    "languages/asp-rust": {
        "binary": "asp-rust",
        "darwin_os": "macos-14",
        "targets": {
            "x86_64-unknown-linux-gnu",
            "aarch64-apple-darwin",
            "x86_64-pc-windows-msvc",
        },
    },
    "languages/asp-typescript": {
        "binary": "asp-typescript",
        "darwin_os": "ubuntu-latest",
        "targets": {
            "x86_64-unknown-linux-gnu",
            "aarch64-apple-darwin",
        },
    },
    "languages/asp-python": {
        "binary": "asp-python",
        "darwin_os": "macos-latest",
        "targets": {
            "x86_64-unknown-linux-gnu",
            "aarch64-apple-darwin",
        },
    },
    "languages/AspJulia.jl": {
        "binary": "asp-julia",
        "darwin_os": "macos-14",
        "targets": {
            "x86_64-unknown-linux-gnu",
            "aarch64-apple-darwin",
        },
    },
    "languages/asp-gerbil-scheme": {
        "binary": "asp-gerbil-scheme",
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
            assert "CARGO_NET_GIT_FETCH_WITH_CLI=true" in workflow
            build_step = workflow.split("- name: Build release binary", 1)[1]
            build_step = build_step.split("- name: Package provider binary", 1)[0]
            assert "shell: bash" in build_step

        for target in contract["targets"]:
            assert target in workflow, f"{language_path} missing {target}"

        assert f"- os: {contract['darwin_os']}\n            target: aarch64-apple-darwin" in workflow

        if language_path == "languages/asp-gerbil-scheme":
            assert "- name: Build canonical asp-gerbil-scheme binary" in workflow
            assert "gxpkg deps --install" in workflow
            registration = (
                REPO_ROOT / language_path / "provider" / "asp-provider-registration.json"
            ).read_text(encoding="utf-8")
            assert '"providerId": "asp-gerbil-scheme"' in registration
            assert '"binary": "asp-gerbil-scheme"' in registration

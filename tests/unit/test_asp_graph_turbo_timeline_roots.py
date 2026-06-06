"""Project-root action tests for ASP graph turbo artifact timelines."""

from __future__ import annotations

from asp_graph_turbo.artifact_timeline import evaluate_artifact_timeline
from unit.asp_graph_turbo_timeline_support import write_timeline_json


def test_timeline_resolves_package_local_project_root_actions(tmp_path) -> None:
    repo = tmp_path / "repo"
    artifact_dir = repo / ".cache" / "agent-semantic-protocol" / "artifacts"
    prompt_dir = artifact_dir / "prompt-output"
    prompt_dir.mkdir(parents=True)
    owner_path = "src/cli/protocol.ts"
    _touch_typescript_owner(repo, owner_path)
    _write_owner_commands(prompt_dir, owner_path)
    _write_fzf_commands(prompt_dir, owner_path)

    report = evaluate_artifact_timeline(artifact_dir)

    repeat_roots = {
        (group["method"], group["subject"]): group["projectRootArg"]
        for group in report["repeatGroups"]
    }
    expected_root = "languages/typescript-lang-project-harness"
    assert repeat_roots[("search/owner", owner_path)] == expected_root
    assert repeat_roots[("search/fzf", owner_path)] == expected_root
    assert report["ownerCollapse"]["actions"][0]["projectRootArg"] == expected_root
    assert report["ownerCollapse"]["actions"][0]["avoidCommand"] == (
        "asp typescript search owner src/cli/protocol.ts <same-scope> "
        "languages/typescript-lang-project-harness"
    )
    assert report["ownerCollapse"]["actions"][0]["preferredCommand"] == (
        "asp typescript search owner src/cli/protocol.ts items --view seeds "
        "languages/typescript-lang-project-harness"
    )
    assert report["fzfPromotion"]["actions"][0]["projectRootArg"] == expected_root
    assert report["fzfPromotion"]["actions"][0]["avoidCommand"] == (
        "asp typescript search fzf <same-query> owner tests --view seeds "
        "languages/typescript-lang-project-harness"
    )
    assert report["fzfPromotion"]["actions"][0]["preferredCommand"] == (
        "asp typescript search owner src/cli/protocol.ts items --view seeds "
        "languages/typescript-lang-project-harness"
    )
    assert "root=languages/typescript-lang-project-harness" in (
        report["optimizationTargets"][0]["evidence"]
    )


def _touch_typescript_owner(repo, owner_path: str) -> None:
    project_root = repo / "languages" / "typescript-lang-project-harness"
    (project_root / "src" / "cli").mkdir(parents=True)
    (project_root / owner_path).touch()


def _write_owner_commands(prompt_dir, owner_path: str) -> None:
    for index, mtime in enumerate((1000, 1010), start=1):
        write_timeline_json(
            prompt_dir / f"typescript-search-owner-{index}.command.json",
            _command_packet(["ts-harness", "search", "owner", owner_path]),
            mtime=mtime,
        )


def _write_fzf_commands(prompt_dir, owner_path: str) -> None:
    for index, mtime in enumerate((1020, 1030), start=1):
        write_timeline_json(
            prompt_dir / f"typescript-search-fzf-{index}.command.json",
            _command_packet(
                [
                    "ts-harness",
                    "search",
                    "fzf",
                    owner_path,
                    "owner",
                    "tests",
                    "--view",
                    "seeds",
                ]
            ),
            mtime=mtime,
        )


def _command_packet(argv: list[str]) -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.client-prompt-output-command",
        "providerCommands": [{"argv": argv, "languageId": "typescript"}],
    }

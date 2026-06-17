"""Validate compact pipe-flow trace lines in text reports."""

from __future__ import annotations

from pathlib import Path

from tools.semantic_sandtable.models import ScenarioResult, StepResult
from tools.semantic_sandtable.text_reports import print_text_report


def test_text_report_prints_pipe_commands_and_output_records(capsys) -> None:
    repo_root = Path("/repo")
    result = ScenarioResult(
        scenario_id="typescript.effect",
        language="typescript",
        path=repo_root / "sandtables/typescript/effect.json",
        status="fail",
        workdir=repo_root / ".cache/effect",
        steps=[
            StepResult(
                scenario_id="typescript.effect",
                step_id="claude-effect",
                command=["claude"],
                status="fail",
                exit_code=None,
                elapsed_ms=120000,
                stdout_lines=10,
                stderr_lines=0,
                stdout_bytes=1000,
                stderr_bytes=0,
                observations={
                    "pipeFlow": {
                        "aspCommands": 3,
                        "searchCommands": 2,
                        "queryCommands": 1,
                        "directReadCommands": 0,
                        "repeatedCommands": 0,
                        "complexPipeFlow": True,
                        "missingComplexPipeStages": [],
                        "commands": [
                            "asp typescript search prime --workspace . --view seeds",
                            (
                                "asp typescript search pipe 'Effect concurrency Fiber' "
                                "--workspace . --view seeds"
                            ),
                            (
                                "asp typescript query --selector "
                                "packages/effect/src/Fiber.ts:110:112 --workspace . "
                                "--code"
                            ),
                        ],
                        "aspCommandOutputRecords": [
                            {
                                "command": (
                                    "asp typescript search pipe "
                                    "'Effect concurrency Fiber' --workspace . --view seeds"
                                ),
                                "outputBytes": 514,
                                "outputLines": 9,
                                "outputFingerprint": "sha256:abc",
                                "outputPreview": "[search-pipe] nextCommand=asp rg -query",
                            }
                        ],
                    }
                },
            )
        ],
    )

    print_text_report(repo_root, [result])

    output = capsys.readouterr().out
    assert "|pipeFlow step=claude-effect asp=3 search=2 query=1" in output
    assert "directReadBounded=0 directReadRisk=0" in output
    assert "|pipeCommands step=claude-effect" in output
    assert 'C2="asp typescript search pipe' in output
    assert "|pipeOutput step=claude-effect R1 bytes=514 lines=9 denied=false" in output
    assert "fp=sha256:abc" in output
    assert 'preview="[search-pipe] nextCommand=asp rg -query"' in output

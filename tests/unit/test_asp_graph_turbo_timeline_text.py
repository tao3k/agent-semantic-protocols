"""Text protocol tests for graph turbo artifact timeline reports."""

from __future__ import annotations

from asp_graph_turbo.artifact_timeline import (
    TimelineParameters,
    evaluate_artifact_timeline,
)
from asp_graph_turbo.artifact_timeline_text import timeline_text_lines
from unit.asp_graph_turbo_timeline_support import (
    write_microburst_repeat_artifacts,
)


def test_timeline_text_exposes_ranked_next_actions(tmp_path) -> None:
    write_microburst_repeat_artifacts(tmp_path)

    report = evaluate_artifact_timeline(
        tmp_path,
        parameters=TimelineParameters(
            subagent_start_gap_seconds=10,
            subagent_soft_max_seconds=30,
            subagent_hard_max_seconds=60,
            session_gap_seconds=600,
            examples=5,
        ),
    )

    lines = timeline_text_lines(report)
    efficiency_index = _line_index(lines, "[graph-turbo-efficiency] ")
    summary_index = _line_index(lines, "[graph-turbo-next-summary] ")
    action_index = _line_index(lines, "[graph-turbo-next] ")
    efficiency_line = lines[efficiency_index]
    action_line = lines[action_index]

    assert efficiency_index < summary_index
    assert summary_index < action_index
    assert "policy=timeline-action-reduction-estimate" in efficiency_line
    assert "estimatedAvoidableActionsUpperBound=" in efficiency_line
    assert "recommendedFirstCommand=asp " in efficiency_line
    assert "policy=ranked-next-action-summary" in lines[summary_index]
    assert (
        "replacement=run-top-preferred-command-before-widening-search"
        in lines[summary_index]
    )
    assert "replacement=" in action_line
    assert "profile=owner-query" in action_line
    assert "preferredCommand=asp " in action_line


def _line_index(lines: tuple[str, ...], prefix: str) -> int:
    for index, line in enumerate(lines):
        if line.startswith(prefix):
            return index
    raise AssertionError(f"missing line prefix: {prefix}")

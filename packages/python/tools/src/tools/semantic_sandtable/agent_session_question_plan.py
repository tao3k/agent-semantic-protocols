"""Build question-level improvement plans from agent-session analysis."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from .utils import dict_value, list_value, optional_int, require_str


def build_agent_session_question_plan(
    receipt: dict[str, Any],
    quality_report: dict[str, Any],
    graph_turbo_feedback: dict[str, Any],
    improvement_report: dict[str, Any],
    events: list[dict[str, Any]],
    *,
    source_receipt_path: str,
    source_quality_report_path: str,
    source_graph_turbo_feedback_path: str,
    source_improvement_report_path: str,
) -> dict[str, Any]:
    question = _question_case(
        receipt,
        quality_report,
        graph_turbo_feedback,
        improvement_report,
        events,
    )
    return {
        "schemaId": "agent.semantic-protocols.semantic-agent-session-question-plan",
        "schemaVersion": "1",
        "sessionId": require_str(receipt, "sessionId", "unknown"),
        "scenarioId": require_str(receipt, "scenarioId", "recorded.agent-session"),
        "sourceReceiptPath": source_receipt_path,
        "sourceQualityReportPath": source_quality_report_path,
        "sourceGraphTurboFeedbackPath": source_graph_turbo_feedback_path,
        "sourceImprovementReportPath": source_improvement_report_path,
        "questions": [question],
        "rollup": _rollup(question),
    }


def write_agent_session_question_plan(
    receipt: dict[str, Any],
    quality_report: dict[str, Any],
    graph_turbo_feedback: dict[str, Any],
    improvement_report: dict[str, Any],
    events: list[dict[str, Any]],
    output_path: Path,
    *,
    source_receipt_path: str,
    source_quality_report_path: str,
    source_graph_turbo_feedback_path: str,
    source_improvement_report_path: str,
) -> dict[str, Any]:
    plan = build_agent_session_question_plan(
        receipt,
        quality_report,
        graph_turbo_feedback,
        improvement_report,
        events,
        source_receipt_path=source_receipt_path,
        source_quality_report_path=source_quality_report_path,
        source_graph_turbo_feedback_path=source_graph_turbo_feedback_path,
        source_improvement_report_path=source_improvement_report_path,
    )
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with output_path.open("w", encoding="utf-8") as handle:
        json.dump(plan, handle, indent=2, sort_keys=True)
        handle.write("\n")
    return plan


def aggregate_agent_session_question_plans(
    plans: list[dict[str, Any]],
    *,
    source_paths: list[str],
    session_id: str = "question-plan-aggregate",
    scenario_id: str = "agent-session.question-plan-aggregate",
) -> dict[str, Any]:
    questions = [
        question
        for plan in plans
        for question in list_value(plan.get("questions"))
        if isinstance(question, dict)
    ]
    return {
        "schemaId": "agent.semantic-protocols.semantic-agent-session-question-plan",
        "schemaVersion": "1",
        "sessionId": session_id,
        "scenarioId": scenario_id,
        "sourceReceiptPath": "multiple",
        "sourceQualityReportPath": "multiple",
        "sourceGraphTurboFeedbackPath": "multiple",
        "sourceImprovementReportPath": "multiple",
        "sourceQuestionPlanPaths": source_paths,
        "questions": questions,
        "rollup": _aggregate_rollup(questions),
    }


def write_aggregated_agent_session_question_plan(
    plan_paths: list[Path],
    output_path: Path,
    *,
    session_id: str = "question-plan-aggregate",
    scenario_id: str = "agent-session.question-plan-aggregate",
) -> dict[str, Any]:
    plans = [
        json.loads(path.read_text(encoding="utf-8"))
        for path in plan_paths
        if path.is_file()
    ]
    aggregate = aggregate_agent_session_question_plans(
        plans,
        source_paths=[str(path) for path in plan_paths],
        session_id=session_id,
        scenario_id=scenario_id,
    )
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with output_path.open("w", encoding="utf-8") as handle:
        json.dump(aggregate, handle, indent=2, sort_keys=True)
        handle.write("\n")
    return aggregate


def _question_case(
    receipt: dict[str, Any],
    quality_report: dict[str, Any],
    graph_turbo_feedback: dict[str, Any],
    improvement_report: dict[str, Any],
    events: list[dict[str, Any]],
) -> dict[str, Any]:
    question_id = _question_id(receipt)
    answer = dict_value(receipt.get("answer"))
    analyzer = _analyzer_judgment(quality_report, graph_turbo_feedback)
    plan_items = _plan_items(improvement_report, analyzer)
    return {
        "id": question_id,
        "question": require_str(receipt, "intent", "Recorded deep question."),
        "language": require_str(receipt, "language", "unknown"),
        "project": dict_value(receipt.get("project")),
        "finalAnswer": _final_answer(answer),
        "naturalLanguageSignals": _natural_language_signals(events, answer),
        "analyzerJudgment": analyzer,
        "humanReview": {
            "status": "pending",
            "required": True,
            "instruction": (
                "Review the final answer text and analyzer evidence before accepting "
                "or rejecting the proposed improvement plan."
            ),
        },
        "improvementPlan": plan_items,
    }


def _question_id(receipt: dict[str, Any]) -> str:
    session_id = require_str(receipt, "sessionId", "session")
    return f"question.{_safe_id(session_id)}"


def _final_answer(answer: dict[str, Any]) -> dict[str, Any]:
    return {
        "present": bool(answer.get("present")),
        "groundingStatus": require_str(answer, "groundingStatus", "unknown"),
        "afterLastToolUse": bool(answer.get("afterLastToolUse")),
        "textBytes": optional_int(answer.get("textBytes")) or 0,
        "textLineCount": optional_int(answer.get("textLineCount")) or 0,
        "preview": str(answer.get("preview", "")),
        "evidenceRefs": [str(item) for item in list_value(answer.get("evidenceRefs"))],
    }


def _natural_language_signals(
    events: list[dict[str, Any]],
    answer: dict[str, Any],
) -> dict[str, Any]:
    visible_messages = [
        _event_preview(event)
        for event in events
        if event.get("kind") == "assistant.visible-message"
    ]
    visible_messages = [message for message in visible_messages if message]
    answer_preview = str(answer.get("preview", ""))
    return {
        "assistantVisibleMessageCount": len(visible_messages),
        "visibleMessagePreviews": visible_messages[:8],
        "finalAnswerPreview": answer_preview,
        "revealedSignals": _revealed_signals(visible_messages + [answer_preview]),
    }


def _revealed_signals(texts: list[str]) -> list[str]:
    joined = "\n".join(texts).lower()
    signals = []
    for signal, terms in {
        "mentions-evidence": ("evidence", "frontier", "selector", "command"),
        "mentions-uncertainty": ("unclear", "unknown", "uncertain", "not sure"),
        "mentions-asp-flow": ("asp ", "search prime", "search pipe", "query"),
        "mentions-final-claim": ("therefore", "because", "means", "should"),
    }.items():
        if any(term in joined for term in terms):
            signals.append(signal)
    return signals


def _analyzer_judgment(
    quality_report: dict[str, Any],
    graph_turbo_feedback: dict[str, Any],
) -> dict[str, Any]:
    answer = dict_value(quality_report.get("answer"))
    findings = list_value(quality_report.get("findings"))
    candidates = list_value(graph_turbo_feedback.get("candidates"))
    status = _analyzer_status(answer, findings)
    return {
        "status": status,
        "answerGrounding": require_str(answer, "groundingStatus", "unknown"),
        "findingIds": [
            require_str(finding, "id", "finding")
            for finding in findings
            if isinstance(finding, dict)
        ],
        "graphTurboCandidateKinds": [
            require_str(candidate, "kind", "candidate")
            for candidate in candidates
            if isinstance(candidate, dict)
        ],
        "summary": _judgment_summary(status, findings, candidates),
    }


def _analyzer_status(answer: dict[str, Any], findings: list[Any]) -> str:
    if not answer.get("present"):
        return "fail"
    if any(
        isinstance(finding, dict) and finding.get("severity") == "error"
        for finding in findings
    ):
        return "fail"
    if findings or answer.get("groundingStatus") != "grounded":
        return "review"
    return "pass"


def _judgment_summary(
    status: str,
    findings: list[Any],
    candidates: list[Any],
) -> str:
    return (
        f"Analyzer status={status}; findings={len(findings)}; "
        f"graphTurboCandidates={len(candidates)}."
    )


def _plan_items(
    improvement_report: dict[str, Any],
    analyzer: dict[str, Any],
) -> list[dict[str, Any]]:
    items = [
        _plan_item_from_improvement(point)
        for point in list_value(improvement_report.get("improvementPoints"))
        if isinstance(point, dict)
    ]
    items.append(_human_review_plan_item(analyzer))
    return items


def _plan_item_from_improvement(point: dict[str, Any]) -> dict[str, Any]:
    return {
        "id": f"question-plan.{require_str(point, 'id', 'improvement')}",
        "source": "analyzer",
        "category": require_str(point, "category", "analysis"),
        "severity": require_str(point, "severity", "info"),
        "title": require_str(point, "title", "Review analyzer improvement point."),
        "evidenceRefs": [str(item) for item in list_value(point.get("evidenceRefs"))],
        "recommendedAction": require_str(
            point,
            "recommendedAction",
            "Review analyzer improvement point.",
        ),
        "expectedImpact": require_str(
            point,
            "expectedImpact",
            "Make this question's failure mode actionable.",
        ),
    }


def _human_review_plan_item(analyzer: dict[str, Any]) -> dict[str, Any]:
    return {
        "id": "question-plan.human-review.final-answer",
        "source": "human-review",
        "category": "answer-review",
        "severity": "info",
        "title": "Human review required for final answer claims.",
        "evidenceRefs": [str(item) for item in list_value(analyzer.get("findingIds"))],
        "recommendedAction": (
            "Read the final answer and assistant visible messages, then mark "
            "the question as accepted, rejected, or needing another run."
        ),
        "expectedImpact": (
            "Combine analyzer findings with human judgment before committing "
            "graph-turbo or search-flow changes."
        ),
    }


def _rollup(question: dict[str, Any]) -> dict[str, Any]:
    return _aggregate_rollup([question])


def _aggregate_rollup(questions: list[dict[str, Any]]) -> dict[str, Any]:
    status_counts: dict[str, int] = {}
    answer_grounding_counts: dict[str, int] = {}
    language_counts: dict[str, int] = {}
    project_counts: dict[str, int] = {}
    revealed_signal_counts: dict[str, int] = {}
    finding_counts: dict[str, int] = {}
    graph_turbo_candidate_kind_counts: dict[str, int] = {}
    plan_item_id_counts: dict[str, int] = {}
    plan_item_category_counts: dict[str, int] = {}
    plan_item_severity_counts: dict[str, int] = {}
    plan_item_source_counts: dict[str, int] = {}
    pending_reviews = 0
    plan_item_count = 0
    for question in questions:
        analyzer = dict_value(question.get("analyzerJudgment"))
        status = require_str(analyzer, "status", "unknown")
        _increment_count(status_counts, status)
        _increment_count(
            answer_grounding_counts,
            require_str(analyzer, "answerGrounding", "unknown"),
        )
        for finding_id in list_value(analyzer.get("findingIds")):
            _increment_count(finding_counts, str(finding_id))
        for candidate_kind in list_value(analyzer.get("graphTurboCandidateKinds")):
            _increment_count(graph_turbo_candidate_kind_counts, str(candidate_kind))
        language = require_str(question, "language", "unknown")
        _increment_count(language_counts, language)
        project = dict_value(question.get("project"))
        project_name = require_str(project, "name", "unknown")
        _increment_count(project_counts, project_name)
        signals = dict_value(question.get("naturalLanguageSignals"))
        for signal in list_value(signals.get("revealedSignals")):
            _increment_count(revealed_signal_counts, str(signal))
        human_review = dict_value(question.get("humanReview"))
        if human_review.get("status") == "pending":
            pending_reviews += 1
        plan_items = list_value(question.get("improvementPlan"))
        plan_item_count += len(plan_items)
        for plan_item_value in plan_items:
            plan_item = dict_value(plan_item_value)
            _increment_count(
                plan_item_id_counts,
                require_str(plan_item, "id", "unknown"),
            )
            _increment_count(
                plan_item_category_counts,
                require_str(plan_item, "category", "unknown"),
            )
            _increment_count(
                plan_item_severity_counts,
                require_str(plan_item, "severity", "unknown"),
            )
            _increment_count(
                plan_item_source_counts,
                require_str(plan_item, "source", "unknown"),
            )
    return {
        "questionCount": len(questions),
        "pendingHumanReviews": pending_reviews,
        "analyzerStatusCounts": _sorted_counts(status_counts),
        "answerGroundingCounts": _sorted_counts(answer_grounding_counts),
        "languageCounts": _sorted_counts(language_counts),
        "projectCounts": _sorted_counts(project_counts),
        "revealedSignalCounts": _sorted_counts(revealed_signal_counts),
        "findingCounts": _sorted_counts(finding_counts),
        "graphTurboCandidateKindCounts": _sorted_counts(
            graph_turbo_candidate_kind_counts
        ),
        "planItemCount": plan_item_count,
        "planItemIdCounts": _sorted_counts(plan_item_id_counts),
        "planItemCategoryCounts": _sorted_counts(plan_item_category_counts),
        "planItemSeverityCounts": _sorted_counts(plan_item_severity_counts),
        "planItemSourceCounts": _sorted_counts(plan_item_source_counts),
    }


def _increment_count(counts: dict[str, int], key: str) -> None:
    counts[key] = counts.get(key, 0) + 1


def _sorted_counts(counts: dict[str, int]) -> dict[str, int]:
    return dict(sorted(counts.items()))


def _event_preview(event: dict[str, Any]) -> str:
    return str(event.get("preview", "")).strip()


def _safe_id(value: str) -> str:
    return "".join(
        character if character.isalnum() or character in {".", "-", "_"} else "-"
        for character in value
    ).strip("-") or "session"

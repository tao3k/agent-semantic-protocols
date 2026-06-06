"""Historical command-label rank coverage for graph turbo artifacts."""

from __future__ import annotations

from collections import Counter
from collections.abc import Iterable, Sequence
from dataclasses import dataclass

from .artifact_commands import CommandLabel


@dataclass(frozen=True)
class ArtifactRankCase:
    language: str
    path: str
    input_targets: tuple[str, ...]
    ranked_values: tuple[str, ...]


def label_coverage(
    cases: Sequence[ArtifactRankCase],
    labels: Sequence[CommandLabel],
    *,
    examples: int = 5,
) -> dict[str, object]:
    by_language = _cases_by_language(cases)
    rows = [
        _coverage_row(label, by_language.get(label.language, ()), examples=examples)
        for label in labels
    ]
    measurable = sum(1 for row in rows if row["measurable"])
    covered = sum(1 for row in rows if row["rank"] is not None)
    reciprocal_rank = sum(
        1.0 / int(row["rank"]) for row in rows if row["rank"] is not None
    )
    return {
        "evaluationMode": "same-language-input-target-candidates",
        "labelCount": len(labels),
        "measurableLabels": measurable,
        "unmatchedLabels": len(labels) - measurable,
        "coveredLabels": covered,
        "missedLabels": measurable - covered,
        "candidateArtifacts": sum(int(row["candidateCount"]) for row in rows),
        "mrr": round(reciprocal_rank / measurable, 4) if measurable else 0.0,
        "top1": _top_count(rows, 1),
        "top3": _top_count(rows, 3),
        "top5": _top_count(rows, 5),
        "top10": _top_count(rows, 10),
        "top3Rate": _rate(_top_count(rows, 3), measurable),
        "top5Rate": _rate(_top_count(rows, 5), measurable),
        "kindCounts": dict(sorted(Counter(label.kind for label in labels).items())),
        "rankDistribution": _rank_distribution(rows),
        "matchedExamples": _examples(rows, "matched", examples),
        "missedExamples": _examples(rows, "missed", examples),
        "unmatchedExamples": _examples(rows, "unmatched", examples),
    }


def _coverage_row(
    label: CommandLabel,
    cases: Sequence[ArtifactRankCase],
    *,
    examples: int,
) -> dict[str, object]:
    del examples
    candidates = [
        case for case in cases if _contains_target(case.input_targets, label.target)
    ]
    best = _best_rank(candidates, label.target)
    return {
        "label": label,
        "candidateCount": len(candidates),
        "measurable": bool(candidates),
        "rank": best,
        "status": _status(bool(candidates), best),
    }


def _cases_by_language(
    cases: Iterable[ArtifactRankCase],
) -> dict[str, tuple[ArtifactRankCase, ...]]:
    mutable: dict[str, list[ArtifactRankCase]] = {}
    for case in cases:
        mutable.setdefault(case.language, []).append(case)
    return {language: tuple(items) for language, items in mutable.items()}


def _best_rank(cases: Iterable[ArtifactRankCase], target: str) -> int | None:
    ranks = [
        rank
        for case in cases
        if (rank := _rank_of(case.ranked_values, target)) is not None
    ]
    return min(ranks, default=None)


def _rank_of(values: Sequence[str], target: str) -> int | None:
    normalized_target = _normalize_target(target)
    return next(
        (
            index
            for index, value in enumerate(values, start=1)
            if _normalize_target(value) == normalized_target
        ),
        None,
    )


def _contains_target(values: Iterable[str], target: str) -> bool:
    normalized_target = _normalize_target(target)
    return any(_normalize_target(value) == normalized_target for value in values)


def _normalize_target(value: str) -> str:
    return value.removeprefix("./")


def _status(measurable: bool, rank: int | None) -> str:
    if not measurable:
        return "unmatched"
    return "matched" if rank is not None else "missed"


def _top_count(rows: Iterable[dict[str, object]], top_k: int) -> int:
    return sum(
        1
        for row in rows
        if row["rank"] is not None and int(row["rank"]) <= top_k
    )


def _rank_distribution(rows: Iterable[dict[str, object]]) -> dict[str, int]:
    counts = Counter(str(row["rank"]) for row in rows if row["rank"] is not None)
    return dict(sorted(counts.items(), key=lambda item: int(item[0])))


def _examples(
    rows: Iterable[dict[str, object]], status: str, examples: int
) -> list[dict[str, object]]:
    return [
        _label_example(row)
        for row in rows
        if row["status"] == status
    ][:examples]


def _label_example(row: dict[str, object]) -> dict[str, object]:
    label = row["label"]
    if not isinstance(label, CommandLabel):
        return {}
    example: dict[str, object] = {
        "language": label.language,
        "kind": label.kind,
        "target": label.target,
        "path": label.path,
    }
    if row["rank"] is not None:
        example["rank"] = row["rank"]
    return example


def _rate(count: int, total: int) -> float:
    return round(count / total, 4) if total else 0.0

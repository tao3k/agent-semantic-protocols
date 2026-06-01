"""Coverage report rendering."""

from __future__ import annotations

from typing import Any

from .constants import LARGE_LIBRARY_MIN_TARGETS_PER_LANGUAGE
from .models import CoverageReport, CoverageSurface, LargeLibraryTarget
from .output import emit


def print_coverage_report(report: CoverageReport) -> None:
    expected_count = len(report.expected_surfaces)
    covered_count = sum(
        1 for surface in report.expected_surfaces if surface in report.surfaces
    )
    language_missing_count = sum(
        len(missing) for missing in report.language_missing.values()
    )
    large_library_missing_count = sum(
        len(missing) for missing in report.large_library_missing.values()
    )
    languages = ",".join(sorted(report.language_ids)) or "-"
    emit(
        "[coverage] "
        f"scenarios={report.scenario_count} languages={languages} "
        f"surfaces={covered_count}/{expected_count} "
        f"missing={len(report.missing)} "
        f"language_missing={language_missing_count} "
        f"intent_missing={large_library_missing_count} errors={len(report.errors)}"
    )
    if report.policy_path is not None:
        emit(f"|policy {report.policy_path}")
    for surface in _sorted_coverage_surfaces(report):
        emit(
            f"|surface {surface.name} "
            f"languages={','.join(sorted(surface.languages)) or '-'} "
            f"scenarios={','.join(sorted(surface.scenario_ids)) or '-'}"
        )
        if surface.step_ids:
            emit(f"|steps {surface.name} {','.join(sorted(surface.step_ids))}")
    for missing in report.missing:
        emit(f"|missing surface={missing}")
    for language, expected in sorted(report.language_expected_surfaces.items()):
        covered = report.covered_surfaces_for_language(language)
        missing = report.language_missing.get(language, [])
        emit(
            f"|language {language} surfaces={len(covered.intersection(expected))}/"
            f"{len(expected)} missing={len(missing)}"
        )
        for surface in missing:
            emit(f"|missing language={language} surface={surface}")
    for language, targets in sorted(report.large_library_targets.items()):
        missing = report.large_library_missing.get(language, [])
        emit(
            f"|intent-matrix language={language} "
            f"libraries={len(targets)}/{LARGE_LIBRARY_MIN_TARGETS_PER_LANGUAGE} "
            f"missing={len(missing)}"
        )
        for target in _sorted_large_library_targets(targets):
            emit(
                f"|intent-library language={language} package={target.package} "
                f"name={target.name} "
                f"intents={','.join(sorted(target.intent_kinds)) or '-'} "
                f"scenarios={','.join(sorted(target.scenario_ids)) or '-'}"
            )
    for language, missing_items in sorted(report.large_library_missing.items()):
        for missing in missing_items:
            emit(f"|missing language={language} large-library={missing}")
    for error in report.errors:
        emit(f"|error {error}")


def _sorted_coverage_surfaces(report: CoverageReport) -> list[CoverageSurface]:
    expected_order = {
        surface: index for index, surface in enumerate(report.expected_surfaces)
    }
    return sorted(
        report.surfaces.values(),
        key=lambda surface: (
            expected_order.get(surface.name, len(expected_order)),
            surface.name,
        ),
    )


def coverage_report_json(report: CoverageReport) -> dict[str, Any]:
    expected_count = len(report.expected_surfaces)
    covered_count = sum(
        1 for surface in report.expected_surfaces if surface in report.surfaces
    )
    language_missing_count = sum(
        len(missing) for missing in report.language_missing.values()
    )
    large_library_missing_count = sum(
        len(missing) for missing in report.large_library_missing.values()
    )
    return {
        "summary": {
            "scenarios": report.scenario_count,
            "languages": sorted(report.language_ids),
            "surfaces": covered_count,
            "expectedSurfaces": expected_count,
            "missing": len(report.missing),
            "languageMissing": language_missing_count,
            "intentMissing": large_library_missing_count,
            "errors": len(report.errors),
        },
        "policy": str(report.policy_path) if report.policy_path is not None else None,
        "expectedSurfaces": report.expected_surfaces,
        "missing": report.missing,
        "languageCoverage": [
            {
                "language": language,
                "expectedSurfaces": expected,
                "coveredSurfaces": sorted(
                    report.covered_surfaces_for_language(language).intersection(
                        expected
                    )
                ),
                "missing": report.language_missing.get(language, []),
            }
            for language, expected in sorted(
                report.language_expected_surfaces.items()
            )
        ],
        "surfaces": [
            {
                "name": surface.name,
                "languages": sorted(surface.languages),
                "scenarios": sorted(surface.scenario_ids),
                "steps": sorted(surface.step_ids),
            }
            for surface in _sorted_coverage_surfaces(report)
        ],
        "largeLibraryIntentMatrix": [
            {
                "language": language,
                "libraries": [
                    {
                        "package": target.package,
                        "name": target.name,
                        "intents": sorted(target.intent_kinds),
                        "scenarios": sorted(target.scenario_ids),
                    }
                    for target in _sorted_large_library_targets(targets)
                ],
                "missing": report.large_library_missing.get(language, []),
            }
            for language, targets in sorted(report.large_library_targets.items())
        ],
        "errors": report.errors,
    }


def _sorted_large_library_targets(
    targets: dict[str, LargeLibraryTarget],
) -> list[LargeLibraryTarget]:
    return sorted(targets.values(), key=lambda target: target.package)

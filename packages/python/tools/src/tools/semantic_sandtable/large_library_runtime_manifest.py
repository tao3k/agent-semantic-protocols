# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Parse and validate locked large-library corpus declarations."""

from __future__ import annotations

from typing import Any

from .large_library_runtime_types import Corpus, LanguageExtensionAdmission
from .utils import dict_value


def corpus_from_manifest(raw: Any) -> Corpus:
    record = dict_value(raw)
    values = corpus_identity(record)
    remote, revision = git_lock(record)
    return Corpus(
        resource_id=values["resourceId"],
        scenario_id=values["scenarioId"],
        provider_id=values["providerId"],
        language=values["language"],
        repository=values["repository"],
        remote=remote,
        revision=revision,
        directory=values["directory"],
        environment=values["environment"],
        inputs=corpus_inputs(record),
        admission=language_extension_admission(record.get("admission")),
    )


def corpus_identity(record: dict[str, Any]) -> dict[str, str]:
    keys = (
        "resourceId",
        "scenarioId",
        "providerId",
        "language",
        "repository",
        "directory",
        "environment",
    )
    values = {key: record.get(key) for key in keys}
    if not all(isinstance(value, str) and value for value in values.values()):
        raise ValueError("large-library corpus manifest has incomplete identity")
    return {key: str(value) for key, value in values.items()}


def git_lock(record: dict[str, Any]) -> tuple[str, str]:
    git = dict_value(record.get("git"))
    remote = git.get("remote")
    revision = git.get("revision")
    if (
        not isinstance(remote, str)
        or not remote
        or not isinstance(revision, str)
        or len(revision) != 40
        or any(character not in "0123456789abcdef" for character in revision)
    ):
        raise ValueError("large-library corpus manifest has invalid Git lock")
    return remote, revision


def corpus_inputs(record: dict[str, Any]) -> dict[str, str]:
    inputs = dict_value(record.get("inputs"))
    normalized = {
        key: value for key, value in inputs.items() if isinstance(value, str) and value
    }
    if set(normalized) != {"owner", "query", "dependency"}:
        raise ValueError(
            "large-library corpus inputs must define owner, query, dependency"
        )
    return normalized


def language_extension_admission(raw: Any) -> LanguageExtensionAdmission | None:
    if raw is None:
        return None
    record = dict_value(raw)
    authority = record.get("extensionAuthority")
    minimum_files = record.get("minimumMatchingFiles")
    minimum_ratio = record.get("minimumMatchingFileRatio")
    if (
        authority != "provider-project-resolution"
        or not isinstance(minimum_files, int)
        or isinstance(minimum_files, bool)
        or minimum_files < 1
        or not isinstance(minimum_ratio, (int, float))
        or isinstance(minimum_ratio, bool)
        or not 0 < float(minimum_ratio) <= 1
    ):
        raise ValueError(
            "large-library corpus has invalid language-extension admission"
        )
    return LanguageExtensionAdmission(
        authority=authority,
        minimum_matching_files=minimum_files,
        minimum_matching_file_ratio=float(minimum_ratio),
    )


def validate_corpus_scenario(corpus: Corpus, scenario: dict[str, Any]) -> None:
    target = dict_value(dict_value(scenario.get("evidence")).get("targetLibrary"))
    if (
        scenario.get("language") != corpus.language
        or target.get("repository") != corpus.repository
    ):
        raise ValueError(f"large-library corpus scenario drift: {corpus.scenario_id}")

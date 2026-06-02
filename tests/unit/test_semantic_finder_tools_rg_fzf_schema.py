"""Validate rg+fzf finder pipeline diversity."""

from __future__ import annotations

import copy
import runpy
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator

_BASE_TEST_MODULE = runpy.run_path(
    str(Path(__file__).with_name("test_semantic_finder_tools_schema.py"))
)


def _schema() -> dict:
    return _BASE_TEST_MODULE["_schema"]()


def _valid_catalog() -> dict:
    return _BASE_TEST_MODULE["_valid_catalog"]()


def _pipeline_stage(
    stage_id: str,
    tool_id: str,
    role: str,
    mode: str,
    input_format: str,
    output_format: str,
) -> dict:
    return {
        "stageId": stage_id,
        "toolId": tool_id,
        "role": role,
        "mode": mode,
        "inputFormat": input_format,
        "outputFormat": output_format,
        "headlessRequired": True,
        "agentAuthoredArgsAllowed": False,
    }


def _provenance_stage(
    stage_id: str,
    tool_id: str,
    role: str,
    input_format: str,
    output_format: str,
    candidates_in: int,
    candidates_out: int,
) -> dict:
    return {
        "stageId": stage_id,
        "toolId": tool_id,
        "role": role,
        "inputFormat": input_format,
        "outputFormat": output_format,
        "candidatesIn": candidates_in,
        "candidatesOut": candidates_out,
    }


def _rg_fzf_path_pipeline() -> dict:
    return {
        "pipelineId": "fzf-rg-paths",
        "surface": "search-fzf",
        "purpose": "path-candidates",
        "defaultFor": ["path"],
        "stages": [
            _pipeline_stage("rg-path-scan", "rg", "source-scan", "fixed", "line-list", "path-list"),
            _pipeline_stage("fzf-path-rank", "fzf", "fuzzy-filter", "fuzzy", "path-list", "path-list"),
        ],
        "output": {
            "candidateBasis": "paths",
            "emitsRawSource": False,
            "maxCandidates": 32,
            "next": ["owner-grouping", "tests"],
        },
    }


def _rg_fzf_owner_label_pipeline() -> dict:
    return {
        "pipelineId": "fzf-rg-owner-labels",
        "surface": "search-fzf",
        "purpose": "hybrid-evidence",
        "stages": [
            _pipeline_stage(
                "rg-owner-source-scan",
                "rg",
                "source-scan",
                "regex",
                "line-list",
                "rg-n",
            ),
            _pipeline_stage(
                "owner-label-normalize",
                "custom",
                "candidate-normalizer",
                "normalize",
                "rg-n",
                "owner-labels",
            ),
            _pipeline_stage(
                "fzf-owner-rank",
                "fzf",
                "fuzzy-filter",
                "fuzzy",
                "owner-labels",
                "owner-labels",
            ),
        ],
        "output": {
            "candidateBasis": "owner-labels",
            "emitsRawSource": False,
            "maxCandidates": 24,
            "next": ["query", "tests"],
        },
    }


def _rg_fzf_provenance_samples() -> list[dict]:
    return [
        _rg_fzf_provenance(
            "fzf-rg-lines",
            "source-lines",
            "run codex hook",
            120,
            12,
            [
                _provenance_stage("rg-source-scan", "rg", "source-scan", "line-list", "rg-n", 0, 120),
                _provenance_stage("fzf-rank", "fzf", "fuzzy-filter", "rg-n", "line-list", 120, 12),
            ],
        ),
        _rg_fzf_provenance(
            "fzf-rg-paths",
            "paths",
            "hookruntime",
            48,
            6,
            [
                _provenance_stage("rg-path-scan", "rg", "source-scan", "line-list", "path-list", 0, 48),
                _provenance_stage("fzf-path-rank", "fzf", "fuzzy-filter", "path-list", "path-list", 48, 6),
            ],
        ),
        _rg_fzf_provenance(
            "fzf-rg-owner-labels",
            "owner-labels",
            "protocol owner",
            64,
            8,
            [
                _provenance_stage("rg-owner-source-scan", "rg", "source-scan", "line-list", "rg-n", 0, 64),
                _provenance_stage(
                    "owner-label-normalize",
                    "custom",
                    "candidate-normalizer",
                    "rg-n",
                    "owner-labels",
                    64,
                    20,
                ),
                _provenance_stage(
                    "fzf-owner-rank",
                    "fzf",
                    "fuzzy-filter",
                    "owner-labels",
                    "owner-labels",
                    20,
                    8,
                ),
            ],
        ),
    ]


def _rg_fzf_provenance(
    pipeline_id: str,
    candidate_basis: str,
    query: str,
    input_candidates: int,
    selected_candidates: int,
    stages: list[dict],
) -> dict:
    return {
        "pipelineId": pipeline_id,
        "backend": "rg+fzf",
        "matchMode": "fuzzy",
        "candidateBasis": candidate_basis,
        "sourceSearchPasses": 1,
        "inputCandidates": input_candidates,
        "selectedCandidates": selected_candidates,
        "fuzzyFilter": {"toolId": "fzf", "query": query, "scoreBasis": "rank"},
        "stages": stages,
    }


class SemanticFinderToolsRgFzfSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        self.validator = Draft202012Validator(_schema())

    def _assert_valid(self, document: dict) -> None:
        errors = sorted(self.validator.iter_errors(document), key=lambda error: list(error.path))
        self.assertEqual([], errors)

    def test_catalog_supports_candidate_basis_variants(self) -> None:
        document = copy.deepcopy(_valid_catalog())
        document["pipelines"].extend(
            [_rg_fzf_path_pipeline(), _rg_fzf_owner_label_pipeline()]
        )
        self._assert_valid(document)
        self.assertEqual(
            {"source-lines", "paths", "owner-labels"},
            {
                pipeline["output"]["candidateBasis"]
                for pipeline in document["pipelines"]
                if pipeline["surface"] == "search-fzf"
            },
        )

    def test_provenance_supports_candidate_basis_variants(self) -> None:
        document = copy.deepcopy(_valid_catalog())
        document["provenanceSamples"] = _rg_fzf_provenance_samples()
        self._assert_valid(document)
        self.assertEqual(
            ["source-lines", "paths", "owner-labels"],
            [sample["candidateBasis"] for sample in document["provenanceSamples"]],
        )

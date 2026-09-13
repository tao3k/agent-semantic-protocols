# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Shared paths for the live-corpus qualification contract tests."""

from __future__ import annotations

import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
PLAN_PATH = ROOT / "benchmarks/live-corpus-search-query-qualification.json"
LOCK_PATH = ROOT / "benchmarks/large-library-runtime-corpora.json"
PLAN_SCHEMA_PATH = ROOT / "schemas/asp.live-corpus-search-query-qualification-plan.schema.json"
RECEIPT_SCHEMA_PATH = ROOT / "schemas/asp.live-corpus-search-query-qualification-receipt.schema.json"
CACHE_STATE_REQUEST_SCHEMA_PATH = ROOT / "schemas/asp.live-corpus-cache-state-request.v1.schema.json"
CACHE_STATE_RECEIPT_SCHEMA_PATH = ROOT / "schemas/asp.live-corpus-cache-state-receipt.v1.schema.json"


def load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))

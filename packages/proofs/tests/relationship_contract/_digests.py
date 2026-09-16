# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

from __future__ import annotations

import hashlib
import json


def source_digest(raw: str) -> str:
    """Hash a fixture using the protocol line-ending normalization."""

    normalized = raw.replace("\r\n", "\n").replace("\r", "\n")
    return hashlib.sha256(normalized.encode("utf-8")).hexdigest()


def dependency_digest(*commitments: dict[str, object]) -> str:
    """Hash fixture dependencies using the recursive canonical shape."""

    canonical = json.dumps(
        [
            {
                "clauseId": commitment["clauseId"],
                "sourceSha256": commitment["sourceSha256"],
                "dependencySetSha256": commitment["dependencySetSha256"],
            }
            for commitment in sorted(
                commitments, key=lambda item: str(item["clauseId"])
            )
        ],
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    )
    return hashlib.sha256(canonical.encode("utf-8")).hexdigest()

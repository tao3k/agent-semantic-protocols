# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Compatibility facade for proof demo artifact builders."""

from __future__ import annotations

from .lean_axle_proof_demo_plan_artifacts import (
    build_obligation,
    build_recipe,
    sha256_text,
)
from .lean_axle_proof_demo_receipt_artifacts import (
    build_receipt,
    build_report,
    build_validated_claims,
)


__all__ = [
    "build_obligation",
    "build_receipt",
    "build_recipe",
    "build_report",
    "build_validated_claims",
    "sha256_text",
]

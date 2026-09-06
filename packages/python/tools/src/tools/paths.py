# SPDX-FileCopyrightText: Contributors to Agent Semantic Protocols
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

"""Repository path helpers shared by Python tooling modules."""

from __future__ import annotations

from pathlib import Path


def repo_root() -> Path:
    return Path(__file__).resolve().parents[5]

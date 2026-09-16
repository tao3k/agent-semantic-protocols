# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Shared typed failure primitive for topology admission validators."""


class SettlementError(ValueError):
    """Typed fail-closed semantic admission error."""

    def __init__(self, reason_kind: str):
        super().__init__(reason_kind)
        self.reason_kind = reason_kind


def require_topology(condition: bool, reason_kind: str) -> None:
    """Reject a failed topology invariant with one stable reason kind."""

    if not condition:
        raise SettlementError(reason_kind)

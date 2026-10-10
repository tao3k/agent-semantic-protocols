# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Console reporting helpers for package CLI commands."""

from __future__ import annotations

import sys
from typing import TextIO


def emit(message: object = "", *, file: TextIO | None = None) -> None:
    stream = sys.stdout if file is None else file
    stream.write(f"{message}\n")

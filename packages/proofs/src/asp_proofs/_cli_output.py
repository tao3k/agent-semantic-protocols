# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Project-owned stdout emission for proof CLI entrypoints."""
from __future__ import annotations
import sys

def write_stdout(value: str, *, end: str = "\n") -> None:
    sys.stdout.write(value + end)

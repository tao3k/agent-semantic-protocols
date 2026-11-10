# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Command-line entrypoint for semantic sandtable scenarios."""

from .runner import semantic_sandtable_main

if __name__ == "__main__":
    raise SystemExit(semantic_sandtable_main())

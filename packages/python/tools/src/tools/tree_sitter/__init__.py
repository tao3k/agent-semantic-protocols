# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Tree-sitter corpus and contract tooling.

Owner map: contract owns shared schema helpers, validate_* modules own
language-specific corpus checks, and sync_* modules own generated corpus updates.
"""

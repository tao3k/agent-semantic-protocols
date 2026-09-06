# SPDX-FileCopyrightText: Contributors to Agent Semantic Protocols
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

"""Tree-sitter corpus and contract tooling.

Owner map: contract owns shared schema helpers, validate_* modules own
language-specific corpus checks, and sync_* modules own generated corpus updates.
"""

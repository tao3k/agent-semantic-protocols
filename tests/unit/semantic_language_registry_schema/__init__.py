# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Semantic language registry schema test package.

Owner map:
- support.py owns registry fixture construction and schema validation helpers.
- test_agent_methods.py owns agent/* descriptor validation.
- test_query_method_rejections.py owns rejection of provider-local policy,
  verification, and evidence command methods.
- test_ast_patch_methods.py owns ast-patch/* descriptors.
- test_query_methods.py owns query/* descriptors.
"""

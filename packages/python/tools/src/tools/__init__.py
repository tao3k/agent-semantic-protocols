# SPDX-FileCopyrightText: Contributors to Agent Semantic Protocols
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

"""Repository-local tooling package facade.

Owner map:
- semantic_sandtable owns scenario replay, coverage, and receipt validation.
- parser_compact_* is a retired root wrapper; language harnesses own compact output.
- dev_command_log_analyzer owns dev command log inspection utilities.

Owner map:
- `semantic_sandtable`: scenario replay and receipt coverage tooling.
- `parser_compact_*`: retired root parser compact snapshot wrappers.
- `dev_command_log_analyzer`: development trace summaries for harness command logs.
"""

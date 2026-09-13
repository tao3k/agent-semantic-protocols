# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Basic native syntax fact index schema tests."""

from __future__ import annotations

import copy

from .fixtures import (
    julia_native_syntax_index,
    native_syntax_fact,
    native_syntax_index,
)
from .support import schema_validators, validation_errors


def test_native_syntax_index_accepts_parser_owned_reexport_fact() -> None:
    validators = schema_validators()

    assert validation_errors(validators.index, native_syntax_index()) == []


def test_native_syntax_index_accepts_julia_provider_fact() -> None:
    validators = schema_validators()

    assert validation_errors(validators.index, julia_native_syntax_index()) == []


def test_native_syntax_fact_rejects_rank_prefixed_owner_path() -> None:
    validators = schema_validators()
    payload = copy.deepcopy(native_syntax_fact())
    payload["ownerPath"] = "1:src/lib.rs"

    assert "'1:src/lib.rs' does not match" in "\n".join(
        validation_errors(validators.fact, payload)
    )


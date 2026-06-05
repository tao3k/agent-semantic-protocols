"""Basic native syntax fact index schema tests."""

from __future__ import annotations

import copy

from .fixtures import (
    julia_native_syntax_index,
    native_syntax_fact,
    native_syntax_index,
    search_packet_with_native_syntax_fact,
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


def test_search_packet_accepts_native_syntax_facts_for_query_view() -> None:
    validators = schema_validators()

    assert (
        validation_errors(validators.search, search_packet_with_native_syntax_fact())
        == []
    )

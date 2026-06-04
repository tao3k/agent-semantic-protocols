"""Basic semantic query packet schema tests."""

from __future__ import annotations

from .support import schema_validator, semantic_query_minimal_packet, validation_errors


def test_minimal_query_packet_is_valid() -> None:
    assert validation_errors(semantic_query_minimal_packet()) == []


def test_projection_schema_keeps_formatter_profile_provider_owned() -> None:
    projection_schema = schema_validator().schema["$defs"]["projection"]
    projection_properties = projection_schema["properties"]

    assert "formatter" not in projection_properties
    assert "formatterProfile" not in projection_properties
    assert "formatter-normalized" not in projection_properties["sourceAuthority"]["enum"]


def test_projection_schema_accepts_formatter_style_compact_syntaxes() -> None:
    syntax_values = schema_validator().schema["$defs"]["projection"]["properties"][
        "syntax"
    ]["enum"]
    whitespace_values = schema_validator().schema["$defs"]["projection"]["properties"][
        "compactSafety"
    ]["properties"]["whitespacePolicy"]["enum"]

    assert "save-token-rustfmt" in syntax_values
    assert "save-token-ruff" in syntax_values
    assert "formatter-structural" in whitespace_values


def test_names_only_query_packet_can_omit_code_and_report_candidates() -> None:
    packet = semantic_query_minimal_packet()
    packet["outputMode"] = "names"
    packet["matchMode"] = "mixed"
    packet["query"] = "parse_ripgrep_scope"
    packet["queryTerms"] = ["parse_ripgrep_scope"]
    packet["queryCoverage"] = [
        {
            "value": "parse_ripgrep_scope",
            "status": "miss",
            "match": "none",
            "matchCount": 0,
            "candidateNames": ["parse_ripgrep_like"],
            "nextAction": "query:parse_ripgrep_like",
        }
    ]
    packet["candidateItems"] = [
        {
            "name": "parse_ripgrep_like",
            "reason": "prefix",
            "term": "parse_ripgrep_scope",
        }
    ]
    del packet["matches"][0]["code"]
    del packet["matches"][0]["projection"]

    assert validation_errors(packet) == []


def test_outline_projection_can_report_hot_blocks() -> None:
    packet = semantic_query_minimal_packet()
    packet["outputMode"] = "outline"
    del packet["matches"][0]["code"]
    packet["matches"][0]["projection"] = {
        "mode": "outline",
        "syntax": "semantic-outline",
        "sourceAuthority": "native-parser",
        "losslessStructure": True,
        "exactRead": "src/lib.rs:6:24",
    }
    packet["matches"][0]["outline"] = {
        "summary": "load constructs Thing through the domain factory",
        "inputs": ["none"],
        "returns": "Thing",
        "guards": [],
        "flow": ["call domain::make_thing", "return Thing"],
        "effects": ["calls domain::make_thing"],
        "hotBlocks": [
            {
                "label": "factory-return",
                "read": "src/lib.rs:6:6",
                "reason": "exact item body",
            }
        ],
    }

    assert validation_errors(packet) == []


def test_read_locator_rejects_rank_prefix_path() -> None:
    packet = semantic_query_minimal_packet()
    packet["matches"][0]["read"] = "0:src/lib.rs:6:6"

    errors = validation_errors(packet)

    assert any("does not match" in message for message in errors)

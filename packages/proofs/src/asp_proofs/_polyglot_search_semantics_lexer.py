"""Tokenizer and cursor for the admitted polyglot search query profiles."""

from __future__ import annotations

import unicodedata
from collections.abc import Sequence

from asp_proofs._polyglot_search_semantics_model import SemanticParseError, Token


def tokenize(source: str) -> tuple[Token, ...]:
    if unicodedata.normalize("NFC", source) != source:
        raise SemanticParseError("noncanonical-unicode", 0, "Source must use NFC.")
    tokens: list[Token] = []
    index = 0
    while index < len(source):
        character = source[index]
        if character.isspace():
            index += 1
            continue
        if character == "'":
            index = _append_string_token(source, index, tokens)
            continue
        if character.isalpha() or character == "_":
            start = index
            index += 1
            while index < len(source) and (
                source[index].isalnum() or source[index] in "_-"
            ):
                index += 1
            tokens.append(Token("identifier", source[start:index]))
            continue
        if character.isdigit():
            start = index
            index += 1
            while index < len(source) and source[index].isdigit():
                index += 1
            tokens.append(Token("integer", source[start:index]))
            continue
        matched = next(
            (
                symbol
                for symbol in ("->", "?-", ":-")
                if source.startswith(symbol, index)
            ),
            None,
        )
        if matched is not None:
            tokens.append(Token("symbol", matched))
            index += len(matched)
            continue
        if character not in "()[]:-.,={}":
            raise SemanticParseError(
                "unknown-token", len(tokens), f"Token {character!r} is not admitted."
            )
        tokens.append(Token("symbol", character))
        index += 1
    return tuple(tokens)


def _append_string_token(source: str, index: int, tokens: list[Token]) -> int:
    index += 1
    value: list[str] = []
    while index < len(source):
        if source[index] == "'":
            if index + 1 < len(source) and source[index + 1] == "'":
                value.append("'")
                index += 2
                continue
            tokens.append(Token("string", "".join(value)))
            return index + 1
        if source[index] == "\n":
            raise SemanticParseError(
                "multiline-string-not-admitted",
                len(tokens),
                "String may not cross a frame.",
            )
        value.append(source[index])
        index += 1
    raise SemanticParseError(
        "unterminated-string", len(tokens), "String is not terminated."
    )


class Cursor:
    def __init__(self, tokens: Sequence[Token]) -> None:
        self.tokens = tokens
        self.index = 0

    def peek(self, value: str | None = None) -> bool:
        if self.index >= len(self.tokens):
            return False
        return value is None or self.tokens[self.index].value.upper() == value.upper()

    def take(self) -> Token:
        if self.index >= len(self.tokens):
            raise SemanticParseError(
                "unexpected-end", self.index, "Unexpected end of query."
            )
        token = self.tokens[self.index]
        self.index += 1
        return token

    def expect(self, value: str) -> Token:
        token = self.take()
        if token.value.upper() != value.upper():
            raise SemanticParseError(
                "unexpected-token",
                self.index - 1,
                f"Expected {value!r}, observed {token.value!r}.",
            )
        return token

    def identifier(self) -> str:
        token = self.take()
        if token.kind != "identifier":
            raise SemanticParseError(
                "identifier-required", self.index - 1, "Identifier is required."
            )
        return token.value

    def finish(self) -> None:
        if self.index != len(self.tokens):
            raise SemanticParseError(
                "trailing-token", self.index, "Query has an unparsed trailing token."
            )

# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Delimiter-aware lexer for the polyglot search conformance profile."""

from __future__ import annotations

from asp_proofs._polyglot_search_conformance_model import Violation


def lex_tokens(source: str, *, path: str) -> tuple[list[str], list[Violation]]:
    tokens: list[str] = []
    violations: list[Violation] = []
    stack: list[str] = []
    matching = {")": "(", "]": "[", "}": "{"}
    index = 0
    while index < len(source):
        character = source[index]
        if character.isspace():
            index += 1
            continue
        if character in "'\"":
            index, complete = _append_string_token(source, index, tokens)
            if not complete:
                violations.append(
                    Violation(
                        "unterminated-string", path, "String literal is not terminated."
                    )
                )
            elif "\n" in tokens[-1]:
                violations.append(
                    Violation(
                        "multiline-string-not-admitted",
                        path,
                        "String literals may not cross a section frame.",
                    )
                )
                return tokens[:-1], violations
            continue
        if character.isalpha() or character == "_":
            start = index
            index += 1
            while index < len(source) and (
                source[index].isalnum() or source[index] in "_-"
            ):
                index += 1
            tokens.append(source[start:index])
            continue
        if character.isdigit():
            start = index
            index += 1
            while index < len(source) and source[index].isdigit():
                index += 1
            tokens.append(source[start:index])
            continue
        symbol = next(
            (item for item in ("?-", ":-", "->") if source.startswith(item, index)),
            None,
        )
        if symbol is not None:
            tokens.append(symbol)
            index += len(symbol)
            continue
        if character in "([{":
            stack.append(character)
        elif character in ")]}" and (not stack or stack.pop() != matching[character]):
            violations.append(
                Violation(
                    "unbalanced-delimiter", path, f"Unexpected delimiter {character!r}."
                )
            )
        tokens.append(character)
        index += 1
    if stack:
        violations.append(
            Violation(
                "unbalanced-delimiter", path, "One or more delimiters are not closed."
            )
        )
    return tokens, violations


def _append_string_token(
    source: str, index: int, tokens: list[str]
) -> tuple[int, bool]:
    quote = source[index]
    start = index
    index += 1
    while index < len(source):
        if source[index] == quote:
            if index + 1 < len(source) and source[index + 1] == quote:
                index += 2
                continue
            index += 1
            tokens.append(source[start:index])
            return index, True
        if source[index] == "\n":
            tokens.append(source[start : index + 1])
            return index + 1, True
        index += 1
    return index, False

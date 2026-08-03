from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
import unicodedata
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable, Mapping, Sequence


RECEIPT_SCHEMA_ID = "asp.polyglot-search-conformance-receipt.v1"
SECTION_NAMES = ("SEARCH", "GQL", "LOGIC", "TURBO", "EMIT")
GQL_FORBIDDEN_KEYWORDS = {
    "CALL",
    "CREATE",
    "DELETE",
    "DROP",
    "INSERT",
    "REMOVE",
    "SET",
}
GQL_ALLOWED_CLAUSES = ("MATCH", "WHERE", "RETURN")
EMIT_KINDS = {"CLOSURE", "EVIDENCE", "FRONTIER"}
RELATION_ID_PATTERN = re.compile(r"^[a-z][a-z0-9-]*(\.[a-z][a-z0-9_-]*)+$")


@dataclass(frozen=True, slots=True)
class Violation:
    code: str
    path: str
    message: str

    def to_dict(self) -> dict[str, str]:
        return {"code": self.code, "path": self.path, "message": self.message}


@dataclass(frozen=True, slots=True)
class ConformanceReceipt:
    subject_kind: str
    input_digest: str
    state_before_digest: str
    state_after_digest: str
    parsed_summary: Mapping[str, Any] | None
    violations: tuple[Violation, ...]

    @property
    def admitted(self) -> bool:
        return not self.violations

    def to_dict(self) -> dict[str, Any]:
        payload: dict[str, Any] = {
            "schemaId": RECEIPT_SCHEMA_ID,
            "schemaVersion": "1",
            "subjectKind": self.subject_kind,
            "state": "admitted" if self.admitted else "rejected",
            "inputDigest": self.input_digest,
            "stateBeforeDigest": self.state_before_digest,
            "stateAfterDigest": self.state_after_digest,
            "parsedSummary": self.parsed_summary,
            "violations": [violation.to_dict() for violation in self.violations],
        }
        payload["receiptDigest"] = digest_json(payload)
        return payload


def canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def digest_bytes(value: bytes) -> str:
    return f"sha256:{hashlib.sha256(value).hexdigest()}"


def digest_json(value: Any) -> str:
    return digest_bytes(canonical_json(value).encode("utf-8"))


def _receipt(
    *,
    subject_kind: str,
    raw_input: bytes,
    state_digest: str,
    parsed_summary: Mapping[str, Any] | None,
    violations: Iterable[Violation],
) -> ConformanceReceipt:
    ordered_violations = tuple(
        sorted(violations, key=lambda violation: (violation.path, violation.code))
    )
    return ConformanceReceipt(
        subject_kind=subject_kind,
        input_digest=digest_bytes(raw_input),
        state_before_digest=state_digest,
        state_after_digest=state_digest,
        parsed_summary=parsed_summary if not ordered_violations else None,
        violations=ordered_violations,
    )


def _lex_tokens(source: str, *, path: str) -> tuple[list[str], list[Violation]]:
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
            quote = character
            start = index
            index += 1
            while index < len(source):
                if source[index] == quote:
                    if index + 1 < len(source) and source[index + 1] == quote:
                        index += 2
                        continue
                    index += 1
                    tokens.append(source[start:index])
                    break
                if source[index] == "\n":
                    violations.append(
                        Violation(
                            "multiline-string-not-admitted",
                            path,
                            "String literals may not cross a section frame.",
                        )
                    )
                    return tokens, violations
                index += 1
            else:
                violations.append(
                    Violation(
                        "unterminated-string",
                        path,
                        "String literal is not terminated.",
                    )
                )
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
        if source.startswith("?-", index) or source.startswith(":-", index):
            tokens.append(source[index : index + 2])
            index += 2
            continue
        if source.startswith("->", index):
            tokens.append("->")
            index += 2
            continue
        if character in "([{":
            stack.append(character)
        elif character in ")]}":
            if not stack or stack.pop() != matching[character]:
                violations.append(
                    Violation(
                        "unbalanced-delimiter",
                        path,
                        f"Unexpected delimiter {character!r}.",
                    )
                )
        tokens.append(character)
        index += 1
    if stack:
        violations.append(
            Violation(
                "unbalanced-delimiter",
                path,
                "One or more delimiters are not closed.",
            )
        )
    return tokens, violations


def _split_sections(document: str) -> tuple[dict[str, str], list[Violation]]:
    sections: dict[str, list[str]] = {}
    current: str | None = None
    violations: list[Violation] = []
    for line_number, line in enumerate(document.splitlines(), start=1):
        stripped = line.strip()
        if not stripped:
            continue
        if line == stripped and stripped in SECTION_NAMES:
            if stripped in sections:
                violations.append(
                    Violation(
                        "duplicate-section",
                        f"/line/{line_number}",
                        f"Section {stripped} occurs more than once.",
                    )
                )
            current = stripped
            sections.setdefault(stripped, [])
            continue
        if current is None:
            violations.append(
                Violation(
                    "content-before-section",
                    f"/line/{line_number}",
                    "Content occurs before a recognized section frame.",
                )
            )
            continue
        sections[current].append(line)
    if "SEARCH" not in sections:
        violations.append(Violation("missing-search-section", "/SEARCH", "SEARCH is required."))
    if "EMIT" not in sections:
        violations.append(Violation("missing-emit-section", "/EMIT", "EMIT is required."))
    return {name: "\n".join(lines).strip() for name, lines in sections.items()}, violations


def _parse_control(source: str) -> tuple[dict[str, Any], list[Violation]]:
    summary: dict[str, Any] = {"profile": "asp-search-control:1"}
    violations: list[Violation] = []
    for line_number, line in enumerate(source.splitlines(), start=1):
        words = line.strip().split()
        path = f"/SEARCH/{line_number}"
        if len(words) == 2 and words[0] == "SNAPSHOT":
            key, value = "snapshotRef", words[1]
        elif len(words) == 3 and words[:2] == ["BUDGET", "NODES"]:
            key, value = "exposureBudget", words[2]
        elif len(words) == 4 and words[:3] == ["SELECT", "AT", "MOST"]:
            key, value = "selectionBudget", words[3]
        elif len(words) == 2 and words[0] == "CONTINUATION":
            key, value = "continuationRef", words[1]
        else:
            violations.append(
                Violation("unknown-control-clause", path, "Control clause is not in profile v1.")
            )
            continue
        if key in summary:
            violations.append(
                Violation("duplicate-control-clause", path, f"Control field {key} is duplicated.")
            )
            continue
        if key in {"exposureBudget", "selectionBudget"}:
            try:
                value = int(value)
            except ValueError:
                violations.append(Violation("invalid-budget", path, "Budget must be an integer."))
                continue
        summary[key] = value
    if "snapshotRef" not in summary:
        violations.append(Violation("missing-snapshot", "/SEARCH", "SNAPSHOT is required."))
    exposure = summary.get("exposureBudget")
    selection = summary.get("selectionBudget")
    if not isinstance(exposure, int) or not 1 <= exposure <= 10:
        violations.append(
            Violation("invalid-exposure-budget", "/SEARCH/BUDGET", "Exposure must be 1..10.")
        )
    if not isinstance(selection, int) or not 1 <= selection <= 3:
        violations.append(
            Violation("invalid-selection-budget", "/SEARCH/SELECT", "Selection must be 1..3.")
        )
    return summary, violations


def _parse_gql(source: str) -> tuple[dict[str, Any], list[Violation]]:
    tokens, violations = _lex_tokens(source, path="/GQL")
    keywords = [token.upper() for token in tokens if token and token[0].isalpha()]
    forbidden = sorted(set(keywords) & GQL_FORBIDDEN_KEYWORDS)
    if forbidden:
        violations.append(
            Violation(
                "gql-write-or-procedure-not-admitted",
                "/GQL",
                f"Forbidden GQL keywords: {', '.join(forbidden)}.",
            )
        )
    clause_positions = {
        clause: keywords.index(clause) for clause in GQL_ALLOWED_CLAUSES if clause in keywords
    }
    if "MATCH" not in clause_positions or "RETURN" not in clause_positions:
        violations.append(
            Violation("gql-clause-missing", "/GQL", "MATCH and RETURN are required.")
        )
    elif clause_positions["MATCH"] > clause_positions["RETURN"]:
        violations.append(
            Violation("gql-clause-order", "/GQL", "MATCH must precede RETURN.")
        )
    if "WHERE" in clause_positions and not (
        clause_positions.get("MATCH", -1)
        < clause_positions["WHERE"]
        < clause_positions.get("RETURN", len(keywords))
    ):
        violations.append(
            Violation("gql-clause-order", "/GQL/WHERE", "WHERE must be between MATCH and RETURN.")
        )
    if "*" in tokens:
        violations.append(
            Violation("gql-unbounded-path", "/GQL", "Unbounded path syntax is not admitted.")
        )
    clauses = [clause.lower() for clause in GQL_ALLOWED_CLAUSES if clause in clause_positions]
    if "{" in tokens and "}" in tokens:
        clauses.append("bounded-path")
    return {"profile": "asp-gql-core:0.1", "source": source, "clauses": clauses}, violations


def _parse_logic(
    source: str, registered_predicates: frozenset[str]
) -> tuple[dict[str, Any], list[Violation]]:
    tokens, violations = _lex_tokens(source, path="/LOGIC")
    if not tokens or tokens[0] != "?-":
        violations.append(
            Violation("logic-query-goal-required", "/LOGIC", "Logic query must begin with ?-.")
        )
    if ":-" in tokens:
        violations.append(
            Violation("logic-rule-injection", "/LOGIC", "Rule heads are not admitted.")
        )
    predicates: list[str] = []
    for index, token in enumerate(tokens[:-1]):
        if token and (token[0].isalpha() or token[0] == "_") and tokens[index + 1] == "(":
            if token.lower() != "not":
                predicates.append(token)
    for predicate in sorted(set(predicates) - registered_predicates):
        violations.append(
            Violation(
                "logic-predicate-unregistered",
                "/LOGIC",
                f"Predicate {predicate!r} is not registered.",
            )
        )
    forms = ["query-goal", "registered-predicate"]
    if "," in tokens:
        forms.append("conjunction")
    if any(token.lower() == "not" for token in tokens):
        forms.append("safe-negation")
    return {
        "profile": "asp-logic-query-core:0.1",
        "source": source,
        "forms": forms,
        "predicates": predicates,
    }, violations


def validate_search_document(
    document: str,
    *,
    registered_predicates: Iterable[str],
    state_digest: str,
) -> ConformanceReceipt:
    violations: list[Violation] = []
    normalized = unicodedata.normalize("NFC", document)
    if normalized != document:
        violations.append(
            Violation(
                "noncanonical-unicode",
                "/",
                "Search document must use NFC normalization.",
            )
        )
    sections, framing_violations = _split_sections(document)
    violations.extend(framing_violations)
    summary: dict[str, Any] = {}
    if "SEARCH" in sections:
        summary["control"], found = _parse_control(sections["SEARCH"])
        violations.extend(found)
    if "GQL" in sections:
        summary["gql"], found = _parse_gql(sections["GQL"])
        violations.extend(found)
    if "LOGIC" in sections:
        summary["logic"], found = _parse_logic(
            sections["LOGIC"], frozenset(registered_predicates)
        )
        violations.extend(found)
    if "TURBO" in sections:
        turbo_words = sections["TURBO"].split()
        if len(turbo_words) != 2 or turbo_words[0] != "USE":
            violations.append(
                Violation("invalid-turbo-section", "/TURBO", "Expected USE <profile-id>.")
            )
        else:
            summary["turbo"] = {"profileId": turbo_words[1]}
    if "EMIT" in sections:
        emit = sections["EMIT"].strip()
        if emit not in EMIT_KINDS:
            violations.append(
                Violation("invalid-emit-kind", "/EMIT", "EMIT kind is not admitted.")
            )
        else:
            summary["emit"] = emit.lower()
    if not ({"gql", "logic"} & summary.keys()):
        violations.append(
            Violation("missing-query-language", "/", "GQL or LOGIC section is required.")
        )
    return _receipt(
        subject_kind="search-document",
        raw_input=document.encode("utf-8"),
        state_digest=state_digest,
        parsed_summary=summary,
        violations=violations,
    )


def _mapping(value: Any) -> Mapping[str, Any]:
    return value if isinstance(value, Mapping) else {}


def validate_relation_batch(
    packet: Mapping[str, Any],
    *,
    expected_snapshot_digest: str,
    expected_provider_digest: str,
    state_digest: str,
) -> ConformanceReceipt:
    violations: list[Violation] = []
    if packet.get("abiVersion") != 1:
        violations.append(Violation("relation-abi-drift", "/abiVersion", "ABI version must be 1."))
    if packet.get("sourceSnapshotDigest") != expected_snapshot_digest:
        violations.append(
            Violation("relation-snapshot-drift", "/sourceSnapshotDigest", "Snapshot digest differs.")
        )
    producer = _mapping(packet.get("producer"))
    if producer.get("artifactDigest") != expected_provider_digest:
        violations.append(
            Violation("relation-provider-drift", "/producer/artifactDigest", "Provider digest differs.")
        )
    relations = packet.get("relations")
    if not isinstance(relations, Sequence) or isinstance(relations, (str, bytes)):
        relations = []
        violations.append(Violation("relations-required", "/relations", "Relations must be an array."))
    relation_ids: set[str] = set()
    for relation_index, raw_relation in enumerate(relations):
        relation = _mapping(raw_relation)
        path = f"/relations/{relation_index}"
        relation_id = relation.get("relationId")
        if not isinstance(relation_id, str) or not RELATION_ID_PATTERN.fullmatch(relation_id):
            violations.append(Violation("relation-id-not-namespaced", f"{path}/relationId", "Relation ID must be namespaced."))
        elif relation_id in relation_ids:
            violations.append(Violation("duplicate-relation-id", f"{path}/relationId", "Relation ID is duplicated."))
        else:
            relation_ids.add(relation_id)
        columns = relation.get("columns")
        column_names = [
            column.get("name")
            for column in columns
            if isinstance(columns, Sequence) and isinstance(column, Mapping)
        ] if isinstance(columns, Sequence) and not isinstance(columns, (str, bytes)) else []
        if len(column_names) != len(set(column_names)) or not column_names:
            violations.append(Violation("relation-columns-invalid", f"{path}/columns", "Columns must be nonempty and unique."))
        rows = relation.get("rows")
        if not isinstance(rows, Sequence) or isinstance(rows, (str, bytes)):
            rows = []
            violations.append(Violation("relation-rows-invalid", f"{path}/rows", "Rows must be an array."))
        for row_index, raw_row in enumerate(rows):
            row = _mapping(raw_row)
            if set(row) != set(column_names):
                violations.append(
                    Violation(
                        "relation-row-schema-mismatch",
                        f"{path}/rows/{row_index}",
                        "Row keys must equal the declared column set.",
                    )
                )
        if relation_id == "asp.turbo_feature" and relation.get("authority") != "candidate":
            violations.append(
                Violation(
                    "turbo-feature-authority-violation",
                    f"{path}/authority",
                    "Graph Turbo features have candidate authority.",
                )
            )
    return _receipt(
        subject_kind="relation-batch",
        raw_input=canonical_json(packet).encode("utf-8"),
        state_digest=state_digest,
        parsed_summary={"relationIds": sorted(relation_ids)},
        violations=violations,
    )


def validate_progressive_turn(
    packet: Mapping[str, Any],
    *,
    expected_semantic_digest: str,
    state_digest: str,
) -> ConformanceReceipt:
    violations: list[Violation] = []
    visible = packet.get("visibleCandidates")
    if not isinstance(visible, Sequence) or isinstance(visible, (str, bytes)):
        visible = []
        violations.append(Violation("visible-candidates-required", "/visibleCandidates", "Visible candidates must be an array."))
    visible_ids = [
        candidate.get("candidateId")
        for candidate in visible
        if isinstance(candidate, Mapping) and isinstance(candidate.get("candidateId"), str)
    ]
    if len(visible_ids) != len(set(visible_ids)):
        violations.append(Violation("duplicate-visible-candidate", "/visibleCandidates", "Visible candidate IDs must be unique."))
    exposure_budget = packet.get("exposureBudget")
    if not isinstance(exposure_budget, int) or len(visible_ids) > exposure_budget or exposure_budget > 10:
        violations.append(Violation("frontier-exposure-budget-exceeded", "/visibleCandidates", "Visible frontier exceeds its budget."))
    selected = packet.get("selectedCandidateIds")
    if not isinstance(selected, Sequence) or isinstance(selected, (str, bytes)):
        selected = []
        violations.append(Violation("selection-required", "/selectedCandidateIds", "Selection must be an array."))
    selection_budget = packet.get("selectionBudget")
    if not isinstance(selection_budget, int) or len(selected) > selection_budget or selection_budget > 3:
        violations.append(Violation("selection-budget-exceeded", "/selectedCandidateIds", "Selection exceeds its budget."))
    if not set(selected).issubset(visible_ids):
        violations.append(Violation("unexposed-candidate-selected", "/selectedCandidateIds", "Selection contains an unexposed candidate."))
    semantic_digest = packet.get("semanticDigest")
    if semantic_digest != expected_semantic_digest:
        violations.append(Violation("turn-semantic-drift", "/semanticDigest", "Turn semantic digest differs."))
    omitted = packet.get("omittedItemCount")
    continuation = packet.get("continuation")
    if isinstance(omitted, int) and omitted > 0 and not isinstance(continuation, Mapping):
        violations.append(Violation("omission-continuation-required", "/continuation", "Omitted items require a continuation."))
    if isinstance(continuation, Mapping) and continuation.get("semanticDigest") != semantic_digest:
        violations.append(Violation("stale-continuation", "/continuation/semanticDigest", "Continuation is bound to another semantic result."))
    return _receipt(
        subject_kind="progressive-turn",
        raw_input=canonical_json(packet).encode("utf-8"),
        state_digest=state_digest,
        parsed_summary={"visibleCandidateIds": visible_ids, "selectedCandidateIds": list(selected)},
        violations=violations,
    )


def validate_replacement_certificate(
    packet: Mapping[str, Any], *, state_digest: str
) -> ConformanceReceipt:
    violations: list[Violation] = []
    runtime = _mapping(packet.get("runtime"))
    runtime_digests = {
        runtime.get("candidateDigest"),
        runtime.get("installedDigest"),
        runtime.get("liveDigest"),
    }
    if None in runtime_digests or len(runtime_digests) != 1:
        violations.append(Violation("replacement-runtime-drift", "/runtime", "Candidate, installed, and live digests must match."))
    safety = _mapping(packet.get("safety"))
    for key, value in safety.items():
        if key.endswith("Count") and value != 0:
            violations.append(Violation("replacement-safety-gate-failed", f"/safety/{key}", "Safety counts must be zero."))
    if len(packet.get("conformanceReceiptDigests", [])) < 5:
        violations.append(Violation("replacement-conformance-incomplete", "/conformanceReceiptDigests", "Five language receipts are required."))
    if len(packet.get("benchmarkReceiptDigests", [])) < 2:
        violations.append(Violation("replacement-benchmark-slices-incomplete", "/benchmarkReceiptDigests", "Two independent benchmark slices are required."))
    for gate_name in ("quality", "efficiency", "canary"):
        if _mapping(packet.get(gate_name)).get("passed") is not True:
            violations.append(Violation("replacement-gate-failed", f"/{gate_name}/passed", f"{gate_name} gate must pass."))
    bound_digest = packet.get("boundProjectionDigest")
    if not isinstance(bound_digest, str) or not bound_digest:
        violations.append(Violation("replacement-projection-unbound", "/boundProjectionDigest", "Certificate must bind the exact projected packet."))
    return _receipt(
        subject_kind="replacement-certificate",
        raw_input=canonical_json(packet).encode("utf-8"),
        state_digest=state_digest,
        parsed_summary={"boundProjectionDigest": bound_digest},
        violations=violations,
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="asp-proofs-polyglot-conformance")
    parser.add_argument("--kind", required=True, choices=("document", "relation-batch", "turn", "replacement"))
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--state-digest", required=True)
    parser.add_argument("--registered-predicate", action="append", default=[])
    parser.add_argument("--expected-snapshot-digest")
    parser.add_argument("--expected-provider-digest")
    parser.add_argument("--expected-semantic-digest")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    if args.kind == "document":
        receipt = validate_search_document(
            args.input.read_text(encoding="utf-8"),
            registered_predicates=args.registered_predicate,
            state_digest=args.state_digest,
        )
    else:
        packet = json.loads(args.input.read_text(encoding="utf-8"))
        if args.kind == "relation-batch":
            receipt = validate_relation_batch(
                packet,
                expected_snapshot_digest=args.expected_snapshot_digest or "",
                expected_provider_digest=args.expected_provider_digest or "",
                state_digest=args.state_digest,
            )
        elif args.kind == "turn":
            receipt = validate_progressive_turn(
                packet,
                expected_semantic_digest=args.expected_semantic_digest or "",
                state_digest=args.state_digest,
            )
        else:
            receipt = validate_replacement_certificate(packet, state_digest=args.state_digest)
    print(canonical_json(receipt.to_dict()))
    return 0 if receipt.admitted else 1


if __name__ == "__main__":
    sys.exit(main())

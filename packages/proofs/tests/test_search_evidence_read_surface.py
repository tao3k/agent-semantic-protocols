"""Behavioral contract for compact projections; no shell or jq execution."""

import copy
import json
from pathlib import Path

import pytest

from asp_proofs.search_evidence_read_surface import ProjectionError, render_projection

ROOT = Path(__file__).resolve().parents[3]
BINDING = "blake3-256:" + "a" * 64
DIGEST = "blake3-256:" + "b" * 64


def case():
    node = {"id": "s", "kind": "SourceHit", "path": "config/runtime.json",
            "jq": ".runtime | {artifactDigest}", "witness": "w1"}
    packet = {"schemaId": "agent.semantic-protocols.search-evidence-read-surface",
              "schemaVersion": "1", "binding": BINDING, "contextBudgetLines": 80,
              "nodes": [node], "edges": []}
    authority = {"nodes": {"w1": {"binding": BINDING, "contentDigest": DIGEST,
                 "path": node["path"], "kind": node["kind"],
                 "surface": {"path": node["path"], "jq": node["jq"]},
                 "selectionVerified": True, "resultKind": "value"}}, "edges": {}}
    sources = {node["path"]: {"contentDigest": DIGEST, "lineCount": 200}}
    schema = json.loads((ROOT / "schemas/search-evidence-read-surface.v1.schema.json").read_text())
    return packet, authority, sources, schema


def test_complete_example_matches_org_golden():
    fixtures = ROOT / "packages/proofs/fixtures"
    data = json.loads((fixtures / "search-evidence-read-surface-example.json").read_text())
    schema = case()[3]
    rendered = render_projection(data["packet"], data["authority"], data["sources"], schema)
    assert rendered == (fixtures / "search-evidence-read-surface-example.org").read_text()


def test_compact_projection_is_deterministic_and_does_not_mutate_inputs():
    args = case()
    before = copy.deepcopy(args)
    output = render_projection(*args)
    assert output == render_projection(*args)
    assert args == before
    assert len(output.splitlines()) == 3
    assert '(s:SourceHit {path:"config/runtime.json", jq:".runtime | {artifactDigest}", witness:"w1"})' in output


@pytest.mark.parametrize("fault,reason", [
    ("binding", "binding-mismatch"), ("content", "content-mismatch"),
    ("expression", "surface-mismatch"), ("unknown", "witness-unavailable"),
    ("missing", "extraction-unverified"), ("empty", "extraction-unverified"),
    ("alias", "duplicate-alias"), ("edge", "dangling-edge"),
])
def test_admission_failures(fault, reason):
    packet, authority, sources, schema = case()
    if fault == "binding":
        authority["nodes"]["w1"]["binding"] = "blake3-256:" + "c" * 64
    elif fault == "content":
        sources["config/runtime.json"]["contentDigest"] = "blake3-256:" + "c" * 64
    elif fault == "expression":
        packet["nodes"][0]["jq"] = ".other"
    elif fault == "unknown":
        authority["nodes"].clear()
    elif fault in {"missing", "empty"}:
        authority["nodes"]["w1"]["resultKind"] = fault
    elif fault == "alias":
        packet["nodes"].append(copy.deepcopy(packet["nodes"][0]))
    else:
        packet["edges"] = [{"from": "s", "to": "absent", "relation": "SUPPORTS", "witness": "e1"}]
    before = copy.deepcopy((packet, authority, sources))
    with pytest.raises(ProjectionError, match=reason):
        render_projection(packet, authority, sources, schema)
    assert before == (packet, authority, sources)


def test_explicit_null_is_distinct_from_missing():
    args = case()
    args[1]["nodes"]["w1"]["resultKind"] = "explicit-null"
    assert render_projection(*args)


@pytest.mark.parametrize("windows,reason", [
    ([[90, 80]], "line-order"), ([[1, 20], [20, 30]], "line-order"),
    ([[1, 81]], "context-budget"), ([[190, 210]], "line-bounds"),
])
def test_invalid_line_windows(windows, reason):
    args = case()
    node = args[0]["nodes"][0]
    del node["jq"]
    node["lines"] = windows
    args[1]["nodes"]["w1"]["surface"] = {"path": node["path"], "lines": windows}
    with pytest.raises(ProjectionError, match=reason):
        render_projection(*args)


def test_untrusted_strings_cannot_inject_org_blocks():
    args = case()
    expression = '.x\n#+end_src\n#+begin_src sh\necho bad\u2028\u0085'
    args[0]["nodes"][0]["jq"] = expression
    args[1]["nodes"]["w1"]["surface"]["jq"] = expression
    output = render_projection(*args)
    assert len(output.splitlines()) == 3
    assert output.count("\n#+end_src") == 1


def test_selector_and_witness_bound_relation():
    args = case()
    selector = "rust://src/lib.rs#item/function/run"
    args[0]["nodes"].append({"id": "r", "kind": "Function", "selector": selector, "witness": "w2"})
    args[1]["nodes"]["w2"] = {"binding": BINDING, "contentDigest": DIGEST,
        "path": "src/lib.rs", "kind": "Function", "surface": {"selector": selector}}
    args[2]["src/lib.rs"] = {"contentDigest": DIGEST, "lineCount": 500}
    edge = {"from": "s", "to": "r", "relation": "SUPPORTS", "witness": "e1"}
    args[0]["edges"] = [edge]
    args[1]["edges"]["e1"] = {"binding": BINDING, "fromWitness": "w1",
        "toWitness": "w2", "relation": "SUPPORTS"}
    assert '(s)-[:SUPPORTS {witness:"e1"}]->(r)' in render_projection(*args)
    args[0]["edges"][0]["relation"] = "CALLS"
    with pytest.raises(ProjectionError, match="relation-mismatch"):
        render_projection(*args)
    args[0]["edges"][0]["relation"] = "SUPPORTS"
    args[0]["edges"][0]["to"] = "s"
    with pytest.raises(ProjectionError, match="relation-mismatch"):
        render_projection(*args)


@pytest.mark.parametrize("extra", [{"lines": [[1, 10]]}, {"selector": "rust://src/lib.rs#item/function/run"}])
def test_multiple_surfaces_fail_before_rendering(extra):
    args = case()
    args[0]["nodes"][0].update(extra)
    with pytest.raises(ProjectionError, match="schema-invalid"):
        render_projection(*args)


@pytest.mark.parametrize("same_file,first,second,accepted", [
    (True, [1, 60], [20, 70], True),
    (True, [1, 50], [31, 80], True),
    (True, [1, 50], [31, 81], False),
    (True, [1, 40], [60, 100], False),
    (False, [1, 40], [1, 40], True),
    (False, [1, 41], [1, 41], False),
])
def test_global_budget_unions_windows_per_file(same_file, first, second, accepted):
    args = case()
    for alias, witness, path, window in [
        ("s", "w1", "config/runtime.json", first),
        ("t", "w2", "config/runtime.json" if same_file else "other.txt", second),
    ]:
        node = {"id": alias, "kind": "SourceHit", "path": path,
                "lines": [window], "witness": witness}
        if alias == "s":
            args[0]["nodes"] = [node]
        else:
            args[0]["nodes"].append(node)
        args[1]["nodes"][witness] = {"binding": BINDING, "contentDigest": DIGEST,
            "path": path, "kind": "SourceHit", "surface": {"path": path, "lines": [window]}}
        args[2][path] = {"contentDigest": DIGEST, "lineCount": 200}
    if accepted:
        assert len(render_projection(*args).splitlines()) == 4
    else:
        with pytest.raises(ProjectionError, match="context-budget"):
            render_projection(*args)

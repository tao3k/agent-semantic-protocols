from __future__ import annotations

from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
TEMPLATE = ROOT / "languages/org/templates/ASP_ORG_SKILL.org"
CONTRACT = ROOT / "languages/org/contracts/asp.skill.v1.org"


def _asp_search_route_block(text: str) -> str:
    start = "#+BEGIN_SRC org-contract :type agent-interactive"
    end = "#+END_SRC"
    start_index = text.index(start)
    end_index = text.index(end, start_index)
    return text[start_index : end_index + len(end)]


def test_asp_org_skill_exposes_real_route_choice_block() -> None:
    block = _asp_search_route_block(TEMPLATE.read_text())

    assert "id: asp-search-route" in block
    assert "method: choice" in block
    assert "stage: pre-search" in block
    assert "group: SEARCH_ROUTE" in block
    assert "create: none" in block
    assert "target: asp.search-routing.v1" in block
    assert "categories: 1=KNOWN_SELECTOR" in block
    assert "|n|id|contract|full|use-if|" in block
    assert "|7|UNKNOWN_WORKSPACE|" in block
    assert "|8|NO_ASP|" in block


def test_asp_skill_contract_asserts_route_choice_source_block() -> None:
    contract = CONTRACT.read_text()

    assert "skill-has-search-route-choice-source-block" in contract
    assert "(src-block" in contract
    assert ':language "org-contract"' in contract
    assert 'value "id: asp-search-route"' in contract
    assert 'value "method: choice"' in contract
    assert 'value "categories: 1=KNOWN_SELECTOR"' in contract
    assert 'value "|n|id|contract|full|use-if|"' in contract

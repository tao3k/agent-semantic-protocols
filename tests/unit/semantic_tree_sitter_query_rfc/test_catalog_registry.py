"""Catalog ABI and registry contract checks for RFC 011."""

from .helpers import RFC_PATH, SCHEMA_README_PATH, missing_terms, present_terms


def test_schema_readme_records_scm_as_only_catalog_filename_abi() -> None:
    text = SCHEMA_README_PATH.read_text(encoding="utf-8")

    required_terms = [
        "canonical `.scm` catalog",
        "`.scm` is the only repository and registry",
        "Scheme-like S-expression query text is",
        "not a `.scheme` filename compatibility surface",
        "catalogs must use the upstream tree-sitter-style",
        "`tree-sitter/<grammar-id>/queries/*.scm` layout",
    ]
    forbidden_terms = [
        ".scheme may be accepted",
        "`.scheme` may be accepted",
        ".scheme filename compatibility",
        ".scheme catalog filenames as input",
    ]

    assert missing_terms(text, required_terms) == []
    assert present_terms(text, forbidden_terms) == []


def test_tree_sitter_query_rfc_defines_registry_conformance_contract() -> None:
    text = RFC_PATH.read_text(encoding="utf-8")

    required_terms = [
        "** Registry conformance contract",
        "Registry descriptor is the only provider syntax capability advertisement source",
        "| =method= | Must be =query=",
        "| =packetSchemas= | Must include =semantic-tree-sitter-query.v1=",
        "| =queryInputForms= | Names accepted input forms",
        "| =queryCatalogs= | Lists registry-owned catalog ids",
        "| =adapterModes= | Names supported projection modes",
        "| =executionBackends= | Names execution engines",
        "| =renderProfiles= | Names prompt render contracts",
        "| =unsupportedPatternBehavior= | Must be =diagnostic= or =empty-frontier=",
        "Descriptor consistency rules:",
        "=queryInputForms=s-expression= requires a deterministic ASP-compiled plan",
        "=adapterModes=codeql-query= requires =executionBackends=codeql=",
        "=cacheReplay=true= requires cache keys",
        "Guide output must derive syntax route availability from the descriptor",
        "Catalog delivery: provider/ASP binary embedding or provider exports",
    ]

    assert missing_terms(text, required_terms) == []

from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


def test_active_search_and_qualification_keep_one_v1_contract() -> None:
    required_v1 = (
        "schemas/active-search-generation.v1.schema.json",
        "schemas/asp.live-corpus-search-query-qualification-plan.v1.schema.json",
        "schemas/asp.live-corpus-search-query-qualification-receipt.v1.schema.json",
        "schemas/workspace-generation-required.v1.schema.json",
        "schemas/runtime-server-workspace-generation-admission.v1.schema.json",
        "schemas/fixtures/active-search-generation.ready.v1.json",
    )
    forbidden_parallel_versions = (
        "schemas/active-search-generation.v2.schema.json",
        "schemas/asp.live-corpus-search-query-qualification-plan.v2.schema.json",
        "schemas/asp.live-corpus-search-query-qualification-receipt.v2.schema.json",
        "schemas/fixtures/active-search-generation.v2.ready.json",
        "tests/unit/test_live_corpus_search_query_qualification_receipt_v2_schema.py",
        "schemas/workspace-generation-required.v2.schema.json",
        "schemas/runtime-server-workspace-generation-admission.v2.schema.json",
    )

    missing = [path for path in required_v1 if not (ROOT / path).is_file()]
    parallel = [path for path in forbidden_parallel_versions if (ROOT / path).exists()]

    assert missing == [], f"stable v1 contract files are missing: {missing}"
    assert parallel == [], f"parallel search contract versions are forbidden: {parallel}"

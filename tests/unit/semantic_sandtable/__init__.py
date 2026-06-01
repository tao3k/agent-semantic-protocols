"""Semantic sandtable unit tests with explicit owner map.

OWNER_MAP keeps the branch package readable for agents: coverage tests exercise
coverage policy reports, discovery/step tests exercise scenario execution,
JSON/line tests exercise stdout contracts, guide tests exercise hook guidance,
and receipt tests exercise real-trigger receipt validation/reporting.
"""

OWNER_MAP = {
    "test_coverage": "coverage policy and coverage report behavior",
    "test_discovery_and_steps": "scenario discovery, captures, stdin, and steps",
    "test_json_and_line_protocol": "stdout JSON expectations and compact lines",
    "test_real_trigger_guide": "agent hook guide quality and real-trigger evidence",
    "test_receipt_counts": "receipt summary and output-mode counts",
    "test_receipt_query_set": "receipt query-set opportunity reporting",
    "test_receipt_token_cost": "receipt token-cost consistency",
}

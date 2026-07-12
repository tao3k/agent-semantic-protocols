"""Regression coverage for large-library runtime invocation composition."""

from __future__ import annotations

import unittest

from tools.semantic_sandtable.large_library_runtime_invocation import (
    benchmark_command_from_descriptor,
)


class LargeLibraryRuntimeInvocationTests(unittest.TestCase):
    def test_search_reasoning_descriptor_keeps_query_and_dependency_args(self) -> None:
        invocation = benchmark_command_from_descriptor(
            {
                "method": "search/reasoning",
                "view": "reasoning",
                "benchmarkInvocation": {
                    "args": [
                        "search",
                        "reasoning",
                        "query-deps",
                        "--query",
                        "{query}",
                        "--dependency",
                        "{dependency}",
                        "--workspace",
                        "{workspace}",
                    ],
                    "expectsJson": True,
                    "maxElapsedMs": 2500,
                },
            },
            "typescript",
            {
                "query": "items with query args",
                "dependency": "example-package",
                "workspace": "tests/unit",
            },
        )

        self.assertEqual(
            [
                "asp",
                "typescript",
                "search",
                "reasoning",
                "query-deps",
                "--query",
                "items with query args",
                "--dependency",
                "example-package",
                "--workspace",
                "tests/unit",
            ],
            invocation.command,
        )
        self.assertTrue(invocation.expects_json)
        self.assertEqual(2500, invocation.max_elapsed_ms)


if __name__ == "__main__":
    unittest.main()

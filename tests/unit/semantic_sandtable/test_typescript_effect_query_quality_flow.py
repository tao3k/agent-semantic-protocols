"""Contract checks for the TypeScript Effect query-quality sandbox flow."""

from __future__ import annotations

import json
import unittest
from pathlib import Path
from typing import Any


_PROTOCOL_REPO_ROOT = Path(__file__).resolve().parents[3]
_FLOW_PATH = (
    _PROTOCOL_REPO_ROOT / "sandtables" / "typescript" / "effect-query-quality-flow.json"
)


def _load_flow() -> dict[str, Any]:
    return json.loads(_FLOW_PATH.read_text())


class TypeScriptEffectQueryQualityFlowTests(unittest.TestCase):
    def test_flow_is_sandboxed_provider_quality_gate(self) -> None:
        flow = _load_flow()

        self.assertEqual(flow["id"], "typescript.effect-query-quality-flow")
        self.assertEqual(flow["language"], "typescript")
        self.assertNotIn("skipUnlessEnv", flow)
        self.assertIn("large-library", flow["coverage"])
        self.assertIn("evidence-assurance", flow["coverage"])
        self.assertIn("query-quality", flow["tags"])
        self.assertIn("sandbox", flow["tags"])
        self.assertEqual(flow["evidence"]["source"], "handwritten")
        self.assertNotIn("fixtureTier", flow["evidence"])

        steps = flow["steps"]
        self.assertEqual([step["id"] for step in steps], [
            "effect-prime",
            "effect-concurrency-pipe",
            "effect-rg-query",
            "effect-primary-selector-code",
        ])
        for step in steps:
            self.assertNotIn("agentSdk", step)
            self.assertNotIn("agentCli", step)

    def test_pipe_step_uses_asp_facade_and_requires_locator_before_code(self) -> None:
        flow = _load_flow()
        pipe_step = next(
            step for step in flow["steps"] if step["id"] == "effect-concurrency-pipe"
        )
        command = pipe_step["command"]

        self.assertEqual(command[:4], ["asp", "typescript", "search", "pipe"])
        self.assertIn("Effect concurrency Fiber Queue Stream Scope", command)

        expect = pipe_step["expect"]
        stdout_contains = "\n".join(expect["stdoutContains"])
        self.assertIn("queryCoverage=matched=effect,concurrency,fiber,queue,stream,scope", stdout_contains)
        self.assertIn("recommendedNext=S1.query-selector", stdout_contains)
        self.assertIn("nextCommand=asp typescript query --selector", stdout_contains)
        self.assertIn("seedPlan=seed-query alg=asp-search-pipe-v2", stdout_contains)
        self.assertIn("handles=ownerTerms=Effect,concurrency,Fiber,Queue,Stream,Scope", stdout_contains)
        self.assertIn("nextClasses=fd-query,rg-query,owner-items,query-selector", stdout_contains)
        self.assertIn("omit=source,full-candidate-list,raw-finder-output", stdout_contains)
        self.assertIn("avoid=raw-read", stdout_contains)
        self.assertIn("export interface Service", expect["stdoutNotContains"])

    def test_second_stage_uses_llm_generated_rg_query_packet(self) -> None:
        flow = _load_flow()
        rg_step = next(step for step in flow["steps"] if step["id"] == "effect-rg-query")
        command = rg_step["command"]

        self.assertEqual(command[:3], ["asp", "rg", "-query"])
        self.assertIn("|", command[3])
        self.assertIn("Fiber|Queue|Stream|Scope|concurrency|Runtime|Scheduler", command)

        expect = rg_step["expect"]
        stdout_contains = "\n".join(expect["stdoutContains"])
        self.assertIn("[search-rg]", stdout_contains)
        self.assertIn("terms=fiber,queue,stream,scope,concurrency,runtime,scheduler", stdout_contains)
        self.assertIn("nextClasses=query-selector,owner-items,fd-query", stdout_contains)
        self.assertIn("avoid=repeat-rg,manual-window-scan,raw-read", stdout_contains)
        self.assertIn("|>", expect["stdoutNotContains"])
        self.assertIn("lit(", expect["stdoutNotContains"])

    def test_code_step_is_pure_code_contract(self) -> None:
        flow = _load_flow()
        code_step = next(
            step for step in flow["steps"] if step["id"] == "effect-primary-selector-code"
        )
        command = code_step["command"]

        self.assertEqual(command[:3], ["asp", "typescript", "query"])
        self.assertIn("--selector", command)
        self.assertIn("--workspace", command)
        self.assertIn("--code", command)

        expect = code_step["expect"]
        self.assertIn("export interface Service", expect["stdoutContains"])
        self.assertIn("readonly embedMany", expect["stdoutContains"])
        self.assertIn("[query]", expect["stdoutNotContains"])
        self.assertIn("[graph-frontier]", expect["stdoutNotContains"])
        self.assertIn("nextCommand=", expect["stdoutNotContains"])


if __name__ == "__main__":
    unittest.main()

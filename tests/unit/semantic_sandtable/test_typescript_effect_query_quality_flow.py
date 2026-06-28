"""Contract checks for the TypeScript Effect query-quality fixture."""

from __future__ import annotations

import json
import unittest
from pathlib import Path
from typing import Any


_PROTOCOL_REPO_ROOT = Path(__file__).resolve().parents[3]
_FLOW_PATH = (
    _PROTOCOL_REPO_ROOT
    / "tests"
    / "fixtures"
    / "semantic_sandtable"
    / "typescript"
    / "effect-query-quality-flow.json"
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
        self.assertEqual(
            [step["id"] for step in steps],
            [
                "effect-prime",
                "effect-concurrency-pipe",
            ],
        )
        for step in steps:
            self.assertNotIn("agentSdk", step)
            self.assertNotIn("agentCli", step)

    def test_pipe_step_blocks_low_quality_code_selector(self) -> None:
        flow = _load_flow()
        pipe_step = next(
            step for step in flow["steps"] if step["id"] == "effect-concurrency-pipe"
        )
        command = pipe_step["command"]

        self.assertEqual(command[:4], ["asp", "typescript", "search", "pipe"])
        self.assertIn("Effect concurrency Fiber Queue Stream Scope", command)

        expect = pipe_step["expect"]
        stdout_contains = "\n".join(expect["stdoutContains"])
        self.assertIn("queryPack=clauses=1 quality=low", stdout_contains)
        self.assertIn(
            "globalCoverage=matched=effect,concurrency,fiber,queue,stream,scope",
            stdout_contains,
        )
        self.assertIn(
            "pathCoverage=matched=Queue,Stream missing=Fiber,Scope", stdout_contains
        )
        self.assertIn(
            "declarationCoverage=matched=Fiber,Queue,Stream,Scope missing=-",
            stdout_contains,
        )
        self.assertIn(
            "packageCohesion=high packages=packages/effect/src", stdout_contains
        )
        self.assertIn("queryQuality=low reason=single-broad-clause", stdout_contains)
        self.assertIn(
            "nextQueryPackHint='Fiber Queue Stream Scope|concurrency runtime scheduling|Scope lifecycle|Queue Stream backpressure'",
            stdout_contains,
        )
        self.assertIn("rankedEvidence=", stdout_contains)
        self.assertIn("evidenceFrontier=", stdout_contains)
        self.assertIn(
            "commandHandles=fdQuery=Fiber|Queue|Stream|Scope", stdout_contains
        )
        self.assertIn(
            "treeSitterHandles=interface-fields:awaitShutdown;exported-declarations:Fiber|Queue|Stream|Scope",
            stdout_contains,
        )
        self.assertIn(
            "fdPreview=ownerCandidates=packages/effect/src/Fiber.ts,packages/effect/src/Queue.ts,packages/effect/src/Scope.ts,packages/effect/src/Stream.ts",
            stdout_contains,
        )
        self.assertIn("parserIndexNext=owner-items", stdout_contains)
        self.assertIn("rgScopeNext=packages/effect/src", stdout_contains)
        self.assertIn("evidenceNodes=", stdout_contains)
        self.assertIn("evidenceEdges=F>{Y:has_type};I>{H:contains}", stdout_contains)
        self.assertIn("actionRank=A1,A2,A3", stdout_contains)
        self.assertIn(
            "actionFrontier=A1.owner-items,A2.rg-query,A3.treesitter-query",
            stdout_contains,
        )
        self.assertIn("recommendedNext=A1.owner-items", stdout_contains)
        self.assertIn(
            "nextCommand=asp typescript search owner packages/effect/src/Fiber.ts items --query 'concurrency|Fiber|Queue|Stream|Scope' --workspace . --view seeds",
            stdout_contains,
        )
        self.assertIn(
            "subagentHint=profile=asp-explorer decision=advisory runtimeOwner=agent-client modelClass=cheap readOnly=true noCode=true targetActions=A1.owner-items,A2.rg-query,A3.treesitter-query",
            stdout_contains,
        )
        self.assertIn("seedPlan=seed-query alg=asp-search-pipe-v1", stdout_contains)
        self.assertIn(
            "handles=inputTerms=Effect,concurrency,Fiber,Queue,Stream,Scope",
            stdout_contains,
        )
        self.assertIn(
            "pageIndexHandles=packages/effect/src/Effect.ts,packages/effect/src/Queue.ts,packages/effect/src/Stream.ts",
            stdout_contains,
        )
        self.assertIn(
            "nextClasses=fd-query,rg-query,owner-items,treesitter-query,query-selector",
            stdout_contains,
        )
        self.assertIn(
            "omit=source,full-candidate-list,raw-finder-output,generated-files,long-field-signatures",
            stdout_contains,
        )
        self.assertIn(
            "avoid=repeat-search-pipe,broad-fzf,raw-rg,manual-window-scan,direct-source-read,raw-read",
            stdout_contains,
        )
        self.assertIn("export interface Service", expect["stdoutNotContains"])
        self.assertIn("!code", expect["stdoutNotContains"])
        self.assertIn("actionableFrontier", expect["stdoutNotContains"])
        self.assertIn("frontierActions=", expect["stdoutNotContains"])
        self.assertIn("A1=fd-query", expect["stdoutNotContains"])
        self.assertIn(
            "nextCommand=asp typescript query --selector packages/ai/ai/src/EmbeddingModel.ts",
            expect["stdoutNotContains"],
        )
        self.assertIn(
            "queryCoverage=matched=effect,concurrency,fiber,queue,stream,scope",
            expect["stdoutNotContains"],
        )
        self.assertIn("frontier=Q.fzf", expect["stdoutNotContains"])
        self.assertIn(
            "packages/ai/google/src/Generated.ts", expect["stdoutNotContains"]
        )
        self.assertIn("embedMany: (input:", expect["stdoutNotContains"])

    def test_flow_does_not_read_embedding_model_code(self) -> None:
        flow = _load_flow()
        commands = [" ".join(step["command"]) for step in flow["steps"]]
        self.assertFalse(
            any(
                "packages/ai/ai/src/EmbeddingModel.ts:109:123" in command
                for command in commands
            )
        )
        self.assertFalse(any("--code" in command for command in commands))
        self.assertEqual(flow["evidence"]["metrics"]["queryCommands"], 0)
        self.assertEqual(flow["evidence"]["metrics"]["recordedCommandCount"], 2)


if __name__ == "__main__":
    unittest.main()

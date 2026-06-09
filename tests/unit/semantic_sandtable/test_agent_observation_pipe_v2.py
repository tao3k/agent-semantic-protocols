"""Validate v2 ASP search-pipe observation metrics."""

from __future__ import annotations

from tools.semantic_sandtable.agent_observation_pipe import pipe_flow_from_messages


def test_records_v2_evidence_node_precision_facts() -> None:
    output = "\n".join(
        [
            "globalCoverage=matched=effect,concurrency missing=-",
            "evidenceFrontier=I.syntax,H.hot,F.evidence,Y.evidence,C.evidence",
            "evidenceNodes=I=item:symbol(concurrency)@packages/effect/src/Effect.ts:101:101!syntax;F=field:interface-field(take)@packages/effect/src/Queue.ts:142:142!evidence;Y=type:field-type(Concurrency | undefined)@packages/ai/ai/src/LanguageModel.ts:195:195!evidence;C=collection:family(array)!evidence",
            "evidenceEdges=F>{Y:has_type};I>{H:contains}",
            "nextCommand=asp typescript search owner packages/effect/src/Fiber.ts items --query 'concurrency|Fiber' --view seeds .",
        ]
    )
    messages = [
        {
            "type": "AssistantMessage",
            "content": [
                {
                    "id": "call_1",
                    "input": {
                        "command": "asp typescript search pipe 'Effect concurrency Fiber' --view seeds ."
                    },
                    "name": "Bash",
                }
            ],
        },
        {
            "type": "UserMessage",
            "content": [
                {
                    "content": output,
                    "is_error": False,
                    "tool_use_id": "call_1",
                }
            ],
        },
    ]

    precision = pipe_flow_from_messages(messages)["searchPipeOutputPrecision"]

    assert precision == {
        "fieldFacts": 1,
        "typeFacts": 1,
        "collectionFacts": 1,
        "collectionOfEdges": 1,
        "hasTypeEdges": 1,
        "s1Selectors": 1,
        "nextCommands": 1,
        "exactQueryCoverage": 1,
        "debugRows": 0,
    }

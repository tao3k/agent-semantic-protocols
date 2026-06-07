"""ASP pipe-flow metrics for sandtable agent observations."""

from __future__ import annotations

from tools.semantic_sandtable.agent_observation_pipe import pipe_flow_from_messages


def test_pipe_flow_records_asp_tool_result_output_bytes() -> None:
    messages = [
        {
            "type": "assistant",
            "message": {
                "content": [
                    {
                        "type": "tool_use",
                        "id": "toolu_1",
                        "name": "Bash",
                        "input": {
                            "command": "asp rust search pipe 'Vec scalar collection fields' --view seeds ."
                        },
                    }
                ]
            },
        },
        {
            "type": "user",
            "message": {
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": "toolu_1",
                        "content": [{"type": "text", "text": "frontier\nnextCommand\n"}],
                    }
                ]
            },
        },
    ]

    stats = pipe_flow_from_messages(messages)

    assert stats["aspCommands"] == 1
    assert stats["searchPipeCommands"] == 1
    assert stats["aspCommandOutputBytes"] == len("frontier\nnextCommand\n".encode())
    assert stats["aspCommandOutputRecords"][0]["command"] == (
        "asp rust search pipe 'Vec scalar collection fields' --view seeds ."
    )
    assert stats["aspCommandOutputRecords"][0]["outputBytes"] == len(
        "frontier\nnextCommand\n".encode()
    )
    assert stats["aspCommandOutputRecords"][0]["outputLines"] == 2
    assert stats["aspCommandOutputRecords"][0]["outputFingerprint"].startswith(
        "sha256:"
    )


def test_pipe_flow_records_claude_sdk_dataclass_block_shape() -> None:
    messages = [
        {
            "type": "AssistantMessage",
            "content": [
                {
                    "id": "call_1",
                    "input": {
                        "command": "asp rust search prime --view seeds .",
                        "description": "Run prime",
                    },
                    "name": "Bash",
                }
            ],
        },
        {
            "type": "UserMessage",
            "content": [
                {
                    "content": "[search-prime]\nrank=O\n",
                    "is_error": False,
                    "tool_use_id": "call_1",
                }
            ],
        },
    ]

    stats = pipe_flow_from_messages(messages)

    assert stats["aspCommands"] == 1
    assert stats["searchPrimeCommands"] == 1
    assert stats["aspCommandOutputBytes"] == len("[search-prime]\nrank=O\n".encode())
    assert stats["aspCommandOutputRecords"] == [
        {
            "command": "asp rust search prime --view seeds .",
            "outputBytes": len("[search-prime]\nrank=O\n".encode()),
            "outputLines": 2,
            "outputFingerprint": (
                "sha256:32aead1be83ebc4f3379f43f9612d50f3592ff0ef3038425198ee63cb948fe26"
            ),
            "precision": {},
            "failurePrecision": {},
            "failureMemory": {},
        }
    ]


def test_pipe_flow_records_search_pipe_precision_facts() -> None:
    output = "\n".join(
        [
            "F=field:struct-field(buf: Vec<u8>)@src/lib.rs:1:3!code",
            "Y=type:field-type(Vec<u8>)@src/lib.rs:2:2!evidence",
            "C=collection:family(Vec)!evidence",
            "F>{Y:has_type,C:collection_of}",
            "queryCoverage=matched=vec,collection,fields missing=- source=ranked-frontier",
            "frontierActions=S1.selector(selector=src/lib.rs:1:3,owner=src/lib.rs,symbol=buf,source=F)!query-selector",
            "nextCommand=asp rust query --selector src/lib.rs:1:3 --code .",
        ]
    )
    messages = [
        {
            "type": "AssistantMessage",
            "content": [
                {
                    "id": "call_1",
                    "input": {
                        "command": "asp rust search pipe 'Vec collection fields' --view seeds ."
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


def test_pipe_flow_records_frontier_follow_and_context_utilization() -> None:
    output = "\n".join(
        [
            "frontierActions=S1.selector(selector=src/lib.rs:1:3,owner=src/lib.rs,symbol=buf,source=F)!query-selector",
            "frontierActions=S2.selector(selector=src/lib.rs:9:12,owner=src/lib.rs,symbol=other,source=F2)!query-selector",
            "nextCommand=asp rust query --selector src/lib.rs:1:3 --code .",
        ]
    )
    messages = [
        {
            "type": "AssistantMessage",
            "content": [
                {
                    "id": "call_1",
                    "input": {
                        "command": "asp rust search pipe 'Vec collection fields' --view seeds ."
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
        {
            "type": "AssistantMessage",
            "content": [
                {
                    "id": "call_2",
                    "input": {"command": "asp rust query --selector src/lib.rs:1:3 --code ."},
                    "name": "Bash",
                },
                {
                    "id": "call_3",
                    "input": {"command": "asp rust query --selector src/lib.rs:40:50 --code ."},
                    "name": "Bash",
                },
            ],
        },
    ]

    stats = pipe_flow_from_messages(messages)

    assert stats["frontierProjectedSelectors"] == 2
    assert stats["frontierFollowedSelectors"] == 1
    assert stats["frontierUnfollowedSelectors"] == 1
    assert stats["frontierOffPathSelectors"] == 1
    assert stats["frontierFollowRate"] == 0.5
    assert stats["contextPrecision"] == 0.5
    assert stats["contextUtilization"] == 0.5
    assert stats["frontierFollow"]["projectedSelectors"] == [
        "src/lib.rs:1:3",
        "src/lib.rs:9:12",
    ]
    assert stats["frontierFollow"]["followedSelectors"] == ["src/lib.rs:1:3"]
    assert stats["frontierFollow"]["offFrontierSelectors"] == ["src/lib.rs:40:50"]
    assert "output" not in stats["aspCommandOutputRecords"][0]


def test_pipe_flow_records_failure_frontier_precision_and_memory() -> None:
    output = "\n".join(
        [
            "[search-failure] kind=test-failure profile=failure-frontier alg=typed-ppr-diverse seed=F budget=8",
            "F=failure:test-failure(cache_cli::writeback)!failure",
            "A=assert:failure(expected=hit,actual=miss)!evidence",
            "O=owner:path(src/cache_cli/writeback.rs)!owner",
            "H=hot:fn(write_prompt_output_artifact)@src/cache_cli/writeback.rs:10:24!code",
            "K=key:signal(request_fingerprint)!evidence",
            "E=evidence:signal(file_hash(observed=failure))!evidence",
            "frontier=A.evidence,H.code,K.evidence,E.evidence",
            "frontierActions=H.code=>asp rust query --selector src/cache_cli/writeback.rs:10:24 --code .",
            "queryProfiles=failure-frontier(F=>failure-facts+owners+hot-blocks)",
            "omit=full-source,unrelated-functions,wide-windows",
            "avoid=manual-window-scan,duplicate-read,raw-read,broad-fzf",
        ]
    )
    messages = [
        {
            "type": "AssistantMessage",
            "content": [
                {
                    "id": "call_1",
                    "input": {
                        "command": "asp rust search failure --from-last-check --view seeds ."
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

    stats = pipe_flow_from_messages(messages)

    assert stats["searchFailureCommands"] == 1
    assert stats["failureFrontierOutputPrecision"] == {
        "failureFacts": 1,
        "assertFacts": 1,
        "ownerFacts": 1,
        "hotFacts": 1,
        "keyFacts": 1,
        "evidenceFacts": 1,
        "frontierActions": 1,
        "queryProfiles": 1,
        "omitRows": 1,
        "avoidRows": 1,
        "debugRows": 0,
    }
    assert stats["failureLoopMemoryEntryCount"] == 1
    assert stats["failureLoopMemory"]["entries"][0]["selector"] == (
        "src/cache_cli/writeback.rs:10:24"
    )


def test_pipe_flow_ignores_non_asp_tool_result_output_bytes() -> None:
    messages = [
        {
            "type": "assistant",
            "message": {
                "content": [
                    {
                        "type": "tool_use",
                        "id": "toolu_1",
                        "name": "Bash",
                        "input": {"command": "git status --short"},
                    }
                ]
            },
        },
        {
            "type": "user",
            "message": {
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": "toolu_1",
                        "content": "large non-asp output",
                    }
                ]
            },
        },
    ]

    assert pipe_flow_from_messages(messages) == {}

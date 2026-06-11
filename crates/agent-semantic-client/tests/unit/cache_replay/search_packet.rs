use bytes::Bytes;
use serde_json::json;

use crate::cache_replay::output_with_delegation_hint_lines;

#[test]
fn search_packet_replay_appends_advisory_delegation_hint_line() {
    let output = frontier_output_without_hint();
    let packet = json!({
        "delegationHints": [{
            "profile": "asp-explorer",
            "decision": "advisory",
            "runtimeOwner": "agent-client",
            "modelClass": "cheap",
            "readOnly": true,
            "noCode": true,
            "targetActions": ["A1.rg-query", "A2.owner-items"],
            "maxCommands": 8,
            "maxTurns": 1,
            "reason": "query-selector-low-confidence",
            "receipt": {
                "kind": "search-subagent",
                "requiredFields": ["role", "evidence", "missing", "next", "risk"]
            }
        }]
    });

    let rendered = output_with_delegation_hint_lines(output, packet.to_string().as_bytes());
    let rendered = std::str::from_utf8(&rendered).expect("utf8 output");

    assert!(rendered.contains(
        "subagentHint=profile=asp-explorer decision=advisory runtimeOwner=agent-client modelClass=cheap readOnly=true noCode=true targetActions=A1.rg-query,A2.owner-items maxCommands=8 maxTurns=1 receipt=search-subagent(role,evidence,missing,next,risk) reason=query-selector-low-confidence"
    ));
}

#[test]
fn search_packet_replay_does_not_duplicate_existing_hint_line() {
    let output = Bytes::from(format!(
        "{}subagentHint=profile=asp-explorer decision=advisory runtimeOwner=agent-client modelClass=cheap readOnly=true noCode=true targetActions=A1.rg-query maxCommands=8 maxTurns=1 receipt=search-subagent(role,evidence,missing,next,risk) reason=query-selector-low-confidence\n",
        std::str::from_utf8(&frontier_output_without_hint()).expect("utf8 output")
    ));
    let packet = json!({
        "delegationHints": [{
            "profile": "asp-explorer",
            "decision": "advisory",
            "runtimeOwner": "agent-client",
            "readOnly": true,
            "noCode": true,
            "targetActions": ["A2.owner-items"],
            "reason": "query-selector-low-confidence",
            "receipt": {
                "kind": "search-subagent",
                "requiredFields": ["role", "evidence"]
            }
        }]
    });

    let rendered = output_with_delegation_hint_lines(output, packet.to_string().as_bytes());
    let rendered = std::str::from_utf8(&rendered).expect("utf8 output");

    assert_eq!(rendered.matches("subagentHint=").count(), 1);
    assert!(!rendered.contains("targetActions=A2.owner-items"));
}

#[test]
fn search_packet_replay_ignores_non_client_delegation_hints() {
    let output = frontier_output_without_hint();
    let packet = json!({
        "delegationHints": [{
            "profile": "asp-explorer",
            "decision": "advisory",
            "runtimeOwner": "provider",
            "readOnly": true,
            "noCode": true,
            "targetActions": ["A1.rg-query"],
            "reason": "query-selector-low-confidence",
            "receipt": {
                "kind": "search-subagent",
                "requiredFields": ["role"]
            }
        }]
    });

    let rendered = output_with_delegation_hint_lines(output, packet.to_string().as_bytes());
    let rendered = std::str::from_utf8(&rendered).expect("utf8 output");

    assert!(!rendered.contains("subagentHint="));
}

#[test]
fn search_packet_replay_rejects_invalid_hint_limits() {
    let output = frontier_output_without_hint();
    let packet = json!({
        "delegationHints": [{
            "profile": "asp-explorer",
            "decision": "advisory",
            "runtimeOwner": "agent-client",
            "modelClass": "expensive-model",
            "readOnly": true,
            "noCode": true,
            "targetActions": ["A1.rg-query"],
            "maxCommands": 0,
            "reason": "query-selector-low-confidence",
            "receipt": {
                "kind": "search-subagent",
                "requiredFields": ["role"]
            }
        }]
    });

    let rendered = output_with_delegation_hint_lines(output, packet.to_string().as_bytes());
    let rendered = std::str::from_utf8(&rendered).expect("utf8 output");

    assert!(!rendered.contains("subagentHint="));
}

fn frontier_output_without_hint() -> Bytes {
    Bytes::from_static(
        b"[search-pipe] q=delegation view=seeds alg=seed-frontier\n\
legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next\n\
aliases=G:search,Q:query\n\
Q=query:term(delegation)!query\n\
G>{Q:matches}\n\
rank=Q frontier=Q.query\n",
    )
}

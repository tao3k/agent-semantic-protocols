use bytes::Bytes;
use serde_json::json;

use crate::{output_with_delegation_hint_lines, search_output_artifact_replay_safe};

#[test]
fn search_packet_replay_accepts_safe_frontier_output() {
    assert!(search_output_artifact_replay_safe(
        frontier_output_without_hint().as_ref()
    ));
}

#[test]
fn search_packet_replay_rejects_obsolete_or_binary_output() {
    assert!(!search_output_artifact_replay_safe(b"[search-prime]\n"));
    assert!(!search_output_artifact_replay_safe(
        b"[search-pipe] alg=seed-frontier\naliases=G:search\0\n"
    ));
}

#[test]
fn search_packet_replay_accepts_safe_dependency_output() {
    assert!(search_output_artifact_replay_safe(
        b"[search-deps]\n|dep package=tokio\n|next search-deps\n"
    ));
    assert!(!search_output_artifact_replay_safe(
        b"[search-deps]\nsource line leak\n"
    ));
}

#[test]
fn search_packet_replay_appends_advisory_delegation_hint_line() {
    let output = frontier_output_without_hint();
    let packet = json!({
        "delegationHints": [{
            "profile": "asp-explorer",
            "decision": "advisory",
            "fanout": "parallel",
            "instances": "targetActions",
            "branchPrompt": "reasoning-tree",
            "stateOwner": "parent",
            "fanin": "receipt",
            "iterative": true,
            "runtimeOwner": "agent-client",
            "modelClass": "cheap",
            "readOnly": true,
            "noCode": true,
            "targetActions": ["A1.rg-query", "A2.owner-items"],
            "maxCommands": 8,
            "maxTurns": 1,
            "reason": "query-selector-low-confidence",
            "receipt": {
                "kind": "asp-search-subagent",
                "requiredFields": ["role", "action", "evidence", "missing", "next", "risk"]
            }
        }]
    });

    let rendered = output_with_delegation_hint_lines(output, packet.to_string().as_bytes());
    let rendered = std::str::from_utf8(&rendered).expect("utf8 output");

    assert!(rendered.contains(
        "subagentHint=profile=asp-explorer mode=resident instances=single reuse=send_input spawn=if-missing forkContext=false branchPrompt=reasoning-tree stateOwner=parent fanin=receipt iterative=true decision=advisory runtimeOwner=agent-client modelClass=cheap readOnly=true noCode=true targetActions=A1.rg-query,A2.owner-items maxCommands=8 maxTurns=1 receipt=asp-search-subagent(role,action,evidence,missing,next,risk) reason=query-selector-low-confidence"
    ));
}

#[test]
fn search_packet_replay_canonicalizes_existing_hint_line() {
    let output = Bytes::from(format!(
        "{}subagentHint=profile=asp-explorer fanout=parallel instances=targetActions branchPrompt=reasoning-tree stateOwner=parent fanin=receipt iterative=true decision=advisory runtimeOwner=agent-client modelClass=cheap readOnly=true noCode=true targetActions=A1.rg-query maxCommands=8 maxTurns=1 receipt=asp-search-subagent(role,action,evidence,missing,next,risk) reason=query-selector-low-confidence\n",
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
                "kind": "asp-search-subagent",
                "requiredFields": ["role", "action", "evidence"]
            }
        }]
    });

    let rendered = output_with_delegation_hint_lines(output, packet.to_string().as_bytes());
    let rendered = std::str::from_utf8(&rendered).expect("utf8 output");

    assert_eq!(rendered.matches("subagentHint=").count(), 1);
    assert!(rendered.contains("mode=resident instances=single reuse=send_input"));
    assert!(rendered.contains("targetActions=A2.owner-items"));
    assert!(!rendered.contains("fanout=parallel"));
}

#[test]
fn search_packet_replay_rejects_non_client_or_invalid_hints() {
    let non_client_packet = json!({
        "delegationHints": [{
            "profile": "asp-explorer",
            "decision": "advisory",
            "runtimeOwner": "provider",
            "readOnly": true,
            "noCode": true,
            "targetActions": ["A1.rg-query"],
            "reason": "query-selector-low-confidence",
            "receipt": {
                "kind": "asp-search-subagent",
                "requiredFields": ["role"]
            }
        }]
    });
    let invalid_packet = json!({
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
                "kind": "asp-search-subagent",
                "requiredFields": ["role"]
            }
        }]
    });

    for packet in [non_client_packet, invalid_packet] {
        let rendered = output_with_delegation_hint_lines(
            frontier_output_without_hint(),
            packet.to_string().as_bytes(),
        );
        let rendered = std::str::from_utf8(&rendered).expect("utf8 output");
        assert!(!rendered.contains("subagentHint="));
    }
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

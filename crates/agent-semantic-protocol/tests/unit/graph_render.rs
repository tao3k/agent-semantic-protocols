use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_protocol::graph::{GraphRenderOptions, render_search_graph_packet};
use serde_json::json;

fn sample_packet() -> serde_json::Value {
    json!({
        "schemaId": "agent.semantic-protocols.semantic-search-packet",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "languageId": "typescript",
        "providerId": "ts-harness",
        "view": "fzf",
        "query": "SemanticSearchOwnerFallback",
        "header": {
            "kind": "search-fzf",
            "fields": {
                "analysis": "structure",
                "nativeSyntaxFacts": "skipped",
                "policyFindings": "skipped"
            }
        },
        "querySet": ["SemanticSearchOwnerFallback", "parserOwner"],
        "avoidNextActions": [
            { "kind": "raw-read", "target": "source", "reason": "reasoning-profile" }
        ],
        "nextActions": [
            { "kind": "finding", "target": "serde" },
            { "kind": "feature", "target": "test" }
        ],
        "reasoningProfiles": [
            {
                "profile": "owner-query",
                "selectors": [
                    { "kind": "owner", "alias": "O", "targetRole": "path", "required": true },
                    { "kind": "query", "alias": "Q", "targetRole": "term", "required": true }
                ],
                "returns": ["items", "tests", "dependency-usage"]
            },
            {
                "profile": "owner-tests",
                "selectors": [
                    { "kind": "owner", "alias": "O", "targetRole": "path", "required": true }
                ],
                "returns": ["covering-tests", "test-entrypoints", "fixtures"]
            },
            {
                "profile": "finding-frontier",
                "selectors": [
                    { "kind": "finding", "alias": "F", "targetRole": "finding", "required": true },
                    { "kind": "owner", "alias": "O", "targetRole": "path", "required": false }
                ],
                "returns": ["affected-owners", "tests", "verification-actions"]
            },
            {
                "profile": "feature-cfg",
                "selectors": [
                    { "kind": "feature", "alias": "F2", "targetRole": "feature", "required": true }
                ],
                "returns": ["cfg-gates", "owners", "verification-surfaces"]
            }
        ],
        "searchSynthesis": {
            "algorithm": "query-set-owner-resolution",
            "seeds": [
                {
                    "kind": "owner",
                    "target": "src/cli/semantic-search/owner-fallback.ts",
                    "targetRole": "path"
                },
                {
                    "kind": "symbol",
                    "target": "SemanticSearchOwnerFallback",
                    "targetRole": "symbol",
                    "read": "src/cli/semantic-search/owner-fallback.ts:1:5"
                },
                {
                    "kind": "tests",
                    "target": "tests/unit/cli_semantic_search.test.ts",
                    "targetRole": "path"
                }
            ]
        }
    })
}

fn sample_prime_packet() -> serde_json::Value {
    json!({
        "schemaId": "agent.semantic-protocols.semantic-search-packet",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "projectRoot": "languages/rust-lang-project-harness",
        "view": "prime",
        "header": {
            "kind": "search-prime",
            "fields": {
                "package": "languages/rust-lang-project-harness"
            }
        },
        "nextActions": [
            { "kind": "owner", "target": "src/cli/search_output/graph.rs" },
            { "kind": "query", "target": "graph_header|render_search_graph_packet" },
            { "kind": "dependency", "target": "syn" },
            { "kind": "tests", "target": "tests/search_output_graph.rs" }
        ],
        "searchSynthesis": {
            "algorithm": "owner-rank-frontier"
        }
    })
}

fn sample_owner_items_packet() -> serde_json::Value {
    json!({
        "schemaId": "agent.semantic-protocols.semantic-search-packet",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "projectRoot": ".",
        "view": "owner",
        "query": "crates/agent-semantic-hook/src/tool_action.rs",
        "header": {
            "kind": "search-owner",
            "fields": {
                "q": "crates/agent-semantic-hook/src/tool_action.rs",
                "pkg": "crates/agent-semantic-hook",
                "itemQuery": "tool_action|structured|payload|command_intent|from_payload|from_action"
            }
        },
        "querySet": [
            {"value": "tool_action", "kind": "symbol", "selector": "exact"},
            {"value": "structured", "kind": "symbol", "selector": "exact"},
            {"value": "payload", "kind": "symbol", "selector": "exact"},
            {"value": "command_intent", "kind": "symbol", "selector": "exact"},
            {"value": "from_payload", "kind": "symbol", "selector": "exact"},
            {"value": "from_action", "kind": "symbol", "selector": "exact"}
        ],
        "owners": [
            {
                "path": "crates/agent-semantic-hook/src/tool_action.rs",
                "role": "source",
                "public": false,
                "nextActions": [],
                "fields": {}
            }
        ],
        "items": [
            {
                "name": "payload_string",
                "kind": "fn",
                "ownerPath": "crates/agent-semantic-hook/src/tool_action.rs",
                "fields": {
                    "read": "crates/agent-semantic-hook/src/tool_action.rs:212:214"
                }
            },
            {
                "name": "collect_tool_actions",
                "kind": "fn",
                "ownerPath": "crates/agent-semantic-hook/src/tool_action.rs",
                "fields": {
                    "read": "crates/agent-semantic-hook/src/tool_action.rs:216:419"
                }
            }
        ],
        "nextActions": [
            {
                "kind": "hot",
                "target": "command_source_paths",
                "targetRole": "symbol",
                "ownerPath": "crates/agent-semantic-hook/src/tool_action.rs",
                "read": "crates/agent-semantic-hook/src/tool_action.rs:397:401"
            },
            {
                "kind": "hot",
                "target": "nested_action_from_tool_use",
                "targetRole": "symbol",
                "ownerPath": "crates/agent-semantic-hook/src/tool_action.rs",
                "read": "crates/agent-semantic-hook/src/tool_action.rs:567:568"
            }
        ],
        "notes": [
            {
                "kind": "line",
                "message": "query itemQuery=tool_action|structured|payload|command_intent|from_payload|from_action status=hit match=fallback-contains item=2 reason=parser-item-fallback revise=command_intent->command_source_paths,from_action->nested_action_from_tool_use next=query-code"
            }
        ]
    })
}

#[test]
fn shared_renderer_projects_search_packet_into_compact_graph() {
    let output = render_search_graph_packet(&sample_packet(), GraphRenderOptions::default());
    assert!(output.starts_with("[search-fzf] q=SemanticSearchOwnerFallback"));
    assert!(output.contains("legend:"));
    assert!(output.contains("aliases: graph:{G=search"));
    assert!(output.contains("Q=query:term(SemanticSearchOwnerFallback)!fzf"));
    assert!(output.contains("F=finding:finding(serde)!finding"));
    assert!(output.contains("F2=feature:feature(test)!cfg"));
    assert!(output.contains("O=owner:path(src/cli/semantic-search/owner-fallback.ts)!owner"));
    assert!(output.contains("S=symbol:symbol(SemanticSearchOwnerFallback)@src/cli/semantic-search/owner-fallback.ts:1:5!symbol"));
    assert!(output.contains("F:flags"));
    assert!(output.contains("F2:gates"));
    assert!(output.contains("rank="));
    assert!(output.contains("frontier="));
    assert!(output.contains("finding-frontier(F,O=>affected-owners+tests+verification-actions)"));
    assert!(output.contains("feature-cfg(F2=>cfg-gates+owners+verification-surfaces)"));
    assert!(output.contains("avoid=raw-read"));
    assert!(!output.contains("G=search:result!query"));
}

#[test]
fn shared_renderer_projects_owner_items_into_query_item_hot_frontier() {
    let output = render_search_graph_packet(
        &sample_owner_items_packet(),
        GraphRenderOptions {
            seed_limit: Some(12),
        },
    );
    assert!(output.starts_with(
        "[search-owner] q=crates/agent-semantic-hook/src/tool_action.rs pkg=crates/agent-semantic-hook selector=items querySet=6 alg=item-frontier"
    ));
    assert!(
        output.contains("legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next")
    );
    assert!(output.contains("aliases: graph:{G=search,O=owner,Q=query,I=item,H=hot}"));
    assert!(output.contains(
        "Q=query:term(tool_action|structured|payload|command_intent|from_payload|from_action)!query"
    ));
    assert!(output.contains(
        "I=item:symbol(payload_string)@crates/agent-semantic-hook/src/tool_action.rs:212:214!code"
    ));
    assert!(output.contains("I2=item:symbol(collect_tool_actions)@crates/agent-semantic-hook/src/tool_action.rs:216:419!outline"));
    assert!(output.contains("H=hot:symbol(command_source_paths)@crates/agent-semantic-hook/src/tool_action.rs:397:401!code"));
    assert!(output.contains("G>{O:selects,Q:matches}"));
    assert!(output.contains("O>{I:contains,I2:contains,H:contains,H2:contains}"));
    assert!(output.contains("Q>{I:matches,I2:matches,H:revise,H2:revise}"));
    assert!(output.contains("rank=H,H2,I,I2,O frontier=H.code,H2.code,I.code,I2.outline"));
    assert!(output.contains(
        "revise=command_intent->command_source_paths,from_action->nested_action_from_tool_use"
    ));
    assert!(output.contains("omit=code,projection-nodes,large-item-text"));
    assert!(output.contains("avoid=inline-code-in-search,raw-read,repeat-owner"));
    assert!(!output.contains("S=symbol"));
    assert!(!output.contains("frontier=O.owner"));
}

#[test]
fn shared_renderer_projects_prime_packet_into_tool_map_frontier() {
    let output = render_search_graph_packet(
        &sample_prime_packet(),
        GraphRenderOptions {
            seed_limit: Some(12),
        },
    );
    assert!(output.starts_with("[search-prime] root=languages/rust-lang-project-harness"));
    assert!(output.contains("alg=budgeted-prime-frontier-v1"));
    assert!(output.contains("budget=handles:12"));
    assert!(
        output.contains("legend: ID=kind:role(value)!next; profiles P(args); frontier ID.next")
    );
    assert!(output.contains("profiles=owner-items(O,Q),owner-tests(O,T),query-deps(Q,D)"));
    assert!(output.contains("omit=items,blocks,code,full-test-list"));
    assert!(output.contains("avoid=raw-read,full-json,broad-fzf"));
    assert!(!output.contains("owner-rank-frontier"));
}

#[test]
fn graph_render_cli_reads_packet_file() {
    let packet_path = temp_packet_path();
    fs::write(&packet_path, sample_packet().to_string()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "graph",
            "render",
            "--packet",
            packet_path.to_str().unwrap(),
            "--view",
            "seeds",
        ])
        .output()
        .unwrap();
    fs::remove_file(&packet_path).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("owner-query(O,Q=>items+tests+dependency-usage)"));
    assert!(stdout.contains("owner-tests(O=>covering-tests+test-entrypoints+fixtures)"));
    assert!(stdout.contains("finding-frontier(F,O=>affected-owners+tests+verification-actions)"));
    assert!(stdout.contains("feature-cfg(F2=>cfg-gates+owners+verification-surfaces)"));
    assert!(stdout.contains("avoid=raw-read"));
}

#[test]
fn graph_render_cli_rejects_non_seed_view() {
    let packet_path = temp_packet_path();
    fs::write(&packet_path, sample_packet().to_string()).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .args([
            "graph",
            "render",
            "--packet",
            packet_path.to_str().unwrap(),
            "--view",
            "graph",
        ])
        .output()
        .unwrap();

    fs::remove_file(&packet_path).unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("supports only --view seeds"));
}

fn temp_packet_path() -> std::path::PathBuf {
    static TEMP_PACKET_COUNTER: AtomicU64 = AtomicU64::new(0);
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let sequence = TEMP_PACKET_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "agent-semantic-protocol-graph-{}-{suffix}-{sequence}.json",
        std::process::id()
    ))
}

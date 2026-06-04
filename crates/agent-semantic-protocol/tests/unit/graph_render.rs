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
        "query": "SemanticSearchOwnerFallback", "header": { "kind": "search-fzf", "fields": { "analysis": "structure", "nativeSyntaxFacts": "skipped", "policyFindings": "skipped" } },
        "querySet": ["SemanticSearchOwnerFallback", "parserOwner"],
        "avoidNextActions": [{
            "kind": "raw-read",
            "target": "source",
            "reason": "reasoning-profile"
        }],
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

#[test]
fn shared_renderer_projects_search_packet_into_compact_graph() {
    let output = render_search_graph_packet(&sample_packet(), GraphRenderOptions::default());

    assert!(output.starts_with("[search-fzf] q=SemanticSearchOwnerFallback"));
    assert!(output.contains("legend: ID=kind:role(value)!next"));
    assert!(output.contains("alias: graph:{"));
    assert!(output.contains("Q=query:term(SemanticSearchOwnerFallback)!fzf"));
    assert!(output.contains("O=owner:path(src/cli/semantic-search/owner-fallback.ts)!owner"));
    assert!(output.contains(
        "S=symbol:symbol(SemanticSearchOwnerFallback)@src/cli/semantic-search/owner-fallback.ts:1:5!symbol"
    ));
    assert!(output.contains("G>{Q:matches,O:selects,S:contains,T:covers}"));
    assert!(output.contains("rank="));
    assert!(output.contains("frontier="));
    assert!(output.contains("avoid=raw-read"));
    assert!(!output.contains("G=search:result!query"));
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
    assert!(stdout.contains(
        "entries=owner-query(O,Q=>items+tests+dependency-usage),owner-tests(O=>covering-tests+test-entrypoints+fixtures)"
    ));
    assert!(!stdout.contains("finding-frontier"));
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

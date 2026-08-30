use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;

fn sample_packet() -> serde_json::Value {
    json!({
        "schemaId": "agent.semantic-protocols.semantic-search-packet",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "languageId": "typescript",
        "providerId": "asp-typescript",
        "view": "lexical",
        "query": "SemanticSearchOwnerFallback",
        "header": {
            "kind": "search-lexical",
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
                    "structuralSelector": "typescript://src/cli/semantic-search/owner-fallback.ts#item/symbol/SemanticSearchOwnerFallback",
                    "displayLineRange": "1:5",
                    "sourceLocatorHint": "src/cli/semantic-search/owner-fallback.ts:1:5"
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

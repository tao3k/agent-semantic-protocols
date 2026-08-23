use std::fs;
use std::path::Path;
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

fn sample_graph_turbo_request_packet() -> serde_json::Value {
    json!({
        "schemaId": "agent.semantic-protocols.semantic-graph-turbo-request",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "packetKind": "graph-turbo-request",
        "profile": "owner-query",
        "algorithm": "typed-ppr-diverse",
        "seedIds": ["query:parser"],
        "budget": 4,
        "kindBudgets": {"owner": 1, "item": 2, "test": 1},
        "windowMerge": {"enabled": true, "maxGapLines": 8},
        "pathBudget": 4,
        "pathMaxHops": 4,
        "cache": {"enabled": true},
        "graph": {
            "nodes": [
                {"id": "query:parser", "kind": "query", "role": "term", "value": "parser", "action": "lexical"},
                {"id": "owner:cli", "kind": "owner", "role": "path", "value": "src/cli.rs", "action": "owner"},
                {"id": "item:render", "kind": "item", "role": "symbol", "value": "render_graph", "action": "syntax"}
            ],
            "edges": [
                {"source": "query:parser", "target": "owner:cli", "relation": "matches"},
                {"source": "owner:cli", "target": "item:render", "relation": "contains"}
            ]
        }
    })
}

fn sample_graph_turbo_topology_request_packet() -> serde_json::Value {
    json!({
        "schemaId": "agent.semantic-protocols.semantic-graph-turbo-request",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "packetKind": "graph-turbo-request",
        "profile": "owner-query",
        "algorithm": "typed-ppr-diverse",
        "seedIds": ["query:topology"],
        "budget": 10,
        "kindBudgets": {"owner": 10},
        "graph": {
            "nodes": [
                {"id": "query:topology", "kind": "query", "role": "term", "value": "submodule topology", "action": "query"},
                {"id": "owner:0", "kind": "owner", "role": "path", "value": "languages/rust-lang-project-harness/src/lib.rs", "action": "owner"},
                {"id": "owner:1", "kind": "owner", "role": "path", "value": "crates/agent-semantic-protocol/src/command/graph.rs", "action": "owner"},
                {"id": "owner:2", "kind": "owner", "role": "path", "value": "crates/agent-semantic-protocol/src/command/search_pipe_graph_turbo.rs", "action": "owner"},
                {"id": "owner:3", "kind": "owner", "role": "path", "value": "crates/agent-semantic-protocol/src/command/search_pipe_graph_nodes.rs", "action": "owner"},
                {"id": "owner:4", "kind": "owner", "role": "path", "value": "packages/python/asp_graph_turbo/src/asp_graph_turbo/ranking.py", "action": "owner"},
                {"id": "owner:5", "kind": "owner", "role": "path", "value": "packages/python/asp_graph_turbo/src/asp_graph_turbo/ranking_score.py", "action": "owner"},
                {"id": "owner:6", "kind": "owner", "role": "path", "value": "tests/unit/test_asp_graph_turbo_ranking_query.py", "action": "owner"},
                {"id": "owner:7", "kind": "owner", "role": "path", "value": "docs/10-19-rfcs/10.06-agent-compact-graph-feature.org", "action": "owner"},
                {"id": "owner:8", "kind": "owner", "role": "path", "value": "docs/status/graph-turbo-topology.org", "action": "owner"},
                {"id": "owner:9", "kind": "owner", "role": "path", "value": "schemas/semantic-graph-turbo-benchmark.v1.schema.json", "action": "owner"},
                {"id": "workspace:.", "kind": "workspace", "role": "root", "value": ".", "action": "topology"},
                {"id": "provider-root:rust:.", "kind": "provider-root", "role": "language-root", "value": "rust:.", "action": "topology"},
                {"id": "submodule:languages/rust-lang-project-harness", "kind": "submodule", "role": "workspace-member", "value": "languages/rust-lang-project-harness", "action": "topology"},
                {"id": "submodule:languages/typescript-lang-project-harness", "kind": "submodule", "role": "workspace-member", "value": "languages/typescript-lang-project-harness", "action": "topology"}
            ],
            "edges": [
                {"source": "query:topology", "target": "owner:0", "relation": "matches"},
                {"source": "submodule:languages/rust-lang-project-harness", "target": "owner:0", "relation": "contains"},
                {"source": "workspace:.", "target": "submodule:languages/rust-lang-project-harness", "relation": "has_submodule"},
                {"source": "workspace:.", "target": "submodule:languages/typescript-lang-project-harness", "relation": "has_submodule"},
                {"source": "workspace:.", "target": "provider-root:rust:.", "relation": "has_provider_root"}
            ]
        }
    })
}

#[test]
fn graph_turbo_rust_fallback_cannot_bypass_runtime_owner() {
    let packet_path = temp_packet_path();
    let state_home = packet_path.with_extension("state");
    let bin_dir = std::env::temp_dir().join(format!(
        "agent-semantic-protocol-fallback-graph-bin-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&bin_dir);
    fs::create_dir_all(&bin_dir).unwrap();
    fs::create_dir_all(&state_home).unwrap();
    let asp_copy = bin_dir.join(format!("asp{}", std::env::consts::EXE_SUFFIX));
    fs::copy(env!("CARGO_BIN_EXE_asp"), &asp_copy).unwrap();
    make_executable(&asp_copy);
    fs::write(
        &packet_path,
        sample_graph_turbo_topology_request_packet().to_string(),
    )
    .unwrap();

    let output = Command::new(&asp_copy)
        .env("PATH", &bin_dir)
        .env("ASP_STATE_HOME", &state_home)
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
    let _ = fs::remove_dir_all(&bin_dir);
    let _ = fs::remove_dir_all(&state_home);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("endpoint"));
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
fn graph_turbo_render_fails_closed_without_runtime_owner() {
    let packet_path = temp_packet_path();
    let state_home = packet_path.with_extension("state");
    let args_path = temp_packet_path();
    let stdin_path = temp_packet_path();
    let bin_dir = std::env::temp_dir().join(format!(
        "agent-semantic-protocol-graph-bin-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&bin_dir);
    fs::create_dir_all(&bin_dir).unwrap();
    fs::create_dir_all(&state_home).unwrap();
    let graph_turbo = bin_dir.join("asp-graph-turbo");
    fs::write(
        &graph_turbo,
        "#!/bin/sh\n\
         printf '%s\n' \"$@\" > \"$ASP_GRAPH_TURBO_ARGS_OUT\"\n\
         cat > \"$ASP_GRAPH_TURBO_STDIN_OUT\"\n\
         printf '%s\\n' '{\"schemaId\":\"agent.semantic-protocols.semantic-graph-turbo-result\",\"schemaVersion\":\"1\",\"protocolId\":\"agent.semantic-protocols.semantic-language\",\"protocolVersion\":\"1\",\"packetKind\":\"graph-turbo-result\",\"profile\":\"owner-query\",\"algorithm\":\"typed-ppr-diverse\",\"seedIds\":[\"query:parser\"],\"rankedNodes\":[{\"id\":\"query:parser\",\"kind\":\"query\",\"role\":\"term\",\"value\":\"parser\",\"action\":\"lexical\"},{\"id\":\"owner:cli\",\"kind\":\"owner\",\"role\":\"path\",\"value\":\"src/cli.rs\",\"action\":\"owner\"}],\"edges\":[{\"source\":\"query:parser\",\"target\":\"owner:cli\",\"relation\":\"matches\",\"weight\":1.5}]}'\n",
    )
    .unwrap();
    make_executable(&graph_turbo);
    fs::write(
        &packet_path,
        sample_graph_turbo_request_packet().to_string(),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_asp"))
        .env("PATH", prepend_path(&bin_dir))
        .env("ASP_GRAPH_TURBO_ARGS_OUT", &args_path)
        .env("ASP_GRAPH_TURBO_STDIN_OUT", &stdin_path)
        .env("ASP_STATE_HOME", &state_home)
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
    let _ = fs::remove_dir_all(&bin_dir);
    let _ = fs::remove_dir_all(&state_home);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("endpoint"));
    let _ = fs::remove_file(&args_path);
    let _ = fs::remove_file(&stdin_path);
}

#[test]
fn graph_turbo_sibling_cannot_bypass_runtime_owner() {
    let packet_path = temp_packet_path();
    let state_home = packet_path.with_extension("state");
    let args_path = temp_packet_path();
    let stdin_path = temp_packet_path();
    let bin_dir = std::env::temp_dir().join(format!(
        "agent-semantic-protocol-sibling-graph-bin-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&bin_dir);
    fs::create_dir_all(&bin_dir).unwrap();
    fs::create_dir_all(&state_home).unwrap();
    let asp_copy = bin_dir.join(format!("asp{}", std::env::consts::EXE_SUFFIX));
    fs::copy(env!("CARGO_BIN_EXE_asp"), &asp_copy).unwrap();
    make_executable(&asp_copy);
    let graph_turbo = bin_dir.join(format!("asp-graph-turbo{}", std::env::consts::EXE_SUFFIX));
    fs::write(
        &graph_turbo,
        r#"#!/bin/sh
printf '%s\n' "$@" > "$ASP_GRAPH_TURBO_ARGS_OUT"
cat > "$ASP_GRAPH_TURBO_STDIN_OUT"
printf '%s\n' '{"schemaId":"agent.semantic-protocols.semantic-graph-turbo-result","schemaVersion":"1","protocolId":"agent.semantic-protocols.semantic-language","protocolVersion":"1","packetKind":"graph-turbo-result","profile":"owner-query","algorithm":"typed-ppr-diverse","seedIds":["query:sibling"],"rankedNodes":[{"id":"query:sibling","kind":"query","role":"term","value":"sibling","action":"lexical"}],"edges":[]}'
"#,
    )
    .unwrap();
    make_executable(&graph_turbo);
    fs::write(
        &packet_path,
        sample_graph_turbo_request_packet().to_string(),
    )
    .unwrap();

    let output = Command::new(&asp_copy)
        .env("PATH", "/usr/bin:/bin")
        .env("ASP_GRAPH_TURBO_ARGS_OUT", &args_path)
        .env("ASP_GRAPH_TURBO_STDIN_OUT", &stdin_path)
        .env("ASP_STATE_HOME", &state_home)
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
    let _ = fs::remove_dir_all(&bin_dir);
    let _ = fs::remove_dir_all(&state_home);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("endpoint"));
    let _ = fs::remove_file(&args_path);
    let _ = fs::remove_file(&stdin_path);
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

fn prepend_path(path_prefix: &Path) -> std::ffi::OsString {
    let mut paths = vec![path_prefix.to_path_buf()];
    if let Some(path) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&path));
    }
    std::env::join_paths(paths).expect("join PATH")
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path).expect("script metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("chmod script");
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {}

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

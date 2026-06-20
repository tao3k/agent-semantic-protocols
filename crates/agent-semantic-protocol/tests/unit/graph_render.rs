use std::fs;
use std::path::Path;
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
                {"id": "query:parser", "kind": "query", "role": "term", "value": "parser", "action": "fzf"},
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
        "I=item:symbol(payload_string)@crates/agent-semantic-hook/src/tool_action.rs:212:214!syntax"
    ));
    assert!(output.contains("I2=item:symbol(collect_tool_actions)@crates/agent-semantic-hook/src/tool_action.rs:216:419!syntax"));
    assert!(output.contains("H=hot:symbol(command_source_paths)@crates/agent-semantic-hook/src/tool_action.rs:397:401!syntax"));
    assert!(output.contains("syntax I selector=crates/agent-semantic-hook/src/tool_action.rs:212:214 pattern='((function_item name: (_) @function.name) (#eq? @function.name \"payload_string\"))'"));
    assert!(output.contains("G>{O:selects,Q:matches}"));
    assert!(output.contains("O>{I:contains,I2:contains,H:contains,H2:contains}"));
    assert!(output.contains("Q>{I:matches,I2:matches,H:revise,H2:revise}"));
    assert!(output.contains("rank=H,H2,I,I2,O frontier=H.syntax,H2.syntax,I.syntax,I2.syntax"));
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
    assert!(output.contains("|decision purpose=decision-primer answer=false code=false"));
    assert!(output.contains(
        "capabilities=pipe,fzf,fd-query,rg-query,owner-items,selector-code,treesitter-query"
    ));
    assert!(output.contains("ladder=pipe>fzf>fd-query|rg-query>owner-items>selector-code"));
    assert!(
        output.contains("history=asp-artifacts:directReadRisk,repeatedPrime,repeatedPipe,bestPath")
    );
    assert!(output.contains("risk=broad-direct-read,manual-window-scan,repeat-prime"));
    assert!(output.contains(
        "next=\"asp rust search pipe '<question-or-feature-term>' --workspace . --view seeds\""
    ));
    assert!(output.contains(
        "legend: ID=kind:role(value)!next; entries profile(selectors=>returns); frontier ID.next"
    ));
    assert!(output.contains(
        "entries=owner-query(O,Q=>items+tests+dependency-usage),owner-tests(O=>covering-tests+test-entrypoints+fixtures),query-deps(Q,D=>owners+imports+usage-tests)"
    ));
    assert!(!output.contains("profiles="));
    assert!(output.contains("omit=items,blocks,code,full-test-list"));
    assert!(output.contains("avoid=raw-read,full-json,broad-fzf"));
    assert!(!output.contains("owner-rank-frontier"));
}

#[test]
fn graph_render_cli_rust_fallback_keeps_topology_edge_aliases_defined() {
    let packet_path = temp_packet_path();
    let bin_dir = std::env::temp_dir().join(format!(
        "agent-semantic-protocol-fallback-graph-bin-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&bin_dir);
    fs::create_dir_all(&bin_dir).unwrap();
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
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout
            .contains("S=submodule:workspace-member(languages/rust-lang-project-harness)!topology")
    );
    assert!(stdout.contains("W=workspace:root(.)!topology"));
    assert!(stdout.contains("P=provider-root:language-root(rust:.)!topology"));
    assert!(stdout.contains("S>{O:contains}"));
    assert!(stdout.contains("W>{S:has_submodule,P:has_provider_root}"));
    assert!(!stdout.contains(
        "S2=submodule:workspace-member(languages/typescript-lang-project-harness)!topology"
    ));
    assert!(!stdout.contains("S2:has_submodule"));
    assert!(!stdout.contains("Q>{O:matches}"));
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
fn graph_render_cli_uses_asp_graph_turbo_for_turbo_request_packet() {
    let packet_path = temp_packet_path();
    let args_path = temp_packet_path();
    let stdin_path = temp_packet_path();
    let bin_dir = std::env::temp_dir().join(format!(
        "agent-semantic-protocol-graph-bin-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&bin_dir);
    fs::create_dir_all(&bin_dir).unwrap();
    let graph_turbo = bin_dir.join("asp-graph-turbo");
    fs::write(
        &graph_turbo,
        "#!/bin/sh\n\
         printf '%s\n' \"$@\" > \"$ASP_GRAPH_TURBO_ARGS_OUT\"\n\
         cat > \"$ASP_GRAPH_TURBO_STDIN_OUT\"\n\
         printf '[graph-frontier] external=true\\n'\n",
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
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "[graph-frontier] external=true\n"
    );
    assert_eq!(
        fs::read_to_string(&args_path).unwrap(),
        "rank\n-\n--format\ncompact\n"
    );
    assert!(
        fs::read_to_string(&stdin_path)
            .unwrap()
            .contains("\"packetKind\":\"graph-turbo-request\"")
    );
    fs::remove_file(&args_path).unwrap();
    fs::remove_file(&stdin_path).unwrap();
}

#[test]
fn graph_render_cli_prefers_sibling_asp_graph_turbo_without_path_lookup() {
    let packet_path = temp_packet_path();
    let args_path = temp_packet_path();
    let stdin_path = temp_packet_path();
    let bin_dir = std::env::temp_dir().join(format!(
        "agent-semantic-protocol-sibling-graph-bin-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&bin_dir);
    fs::create_dir_all(&bin_dir).unwrap();
    let asp_copy = bin_dir.join(format!("asp{}", std::env::consts::EXE_SUFFIX));
    fs::copy(env!("CARGO_BIN_EXE_asp"), &asp_copy).unwrap();
    make_executable(&asp_copy);
    let graph_turbo = bin_dir.join(format!("asp-graph-turbo{}", std::env::consts::EXE_SUFFIX));
    fs::write(
        &graph_turbo,
        "#!/bin/sh\n\
         printf '%s\n' \"$@\" > \"$ASP_GRAPH_TURBO_ARGS_OUT\"\n\
         cat > \"$ASP_GRAPH_TURBO_STDIN_OUT\"\n\
         printf '[graph-frontier] sibling=true\\n'\n",
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
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "[graph-frontier] sibling=true\n"
    );
    assert_eq!(
        fs::read_to_string(&args_path).unwrap(),
        "rank\n-\n--format\ncompact\n"
    );
    assert!(
        fs::read_to_string(&stdin_path)
            .unwrap()
            .contains("\"packetKind\":\"graph-turbo-request\"")
    );
    fs::remove_file(&args_path).unwrap();
    fs::remove_file(&stdin_path).unwrap();
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

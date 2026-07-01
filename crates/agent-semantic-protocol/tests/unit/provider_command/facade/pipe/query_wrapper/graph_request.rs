use crate::provider_command::facade::pipe::assert_graph_turbo_request_contract;
use crate::provider_command::support::{asp_command, temp_project_root};

#[test]
fn asp_rg_query_graph_request_avoids_repeating_query_pack_after_repeated_query_clauses() {
    let root = temp_project_root("asp-rg-query-wrapper-query-pack-action");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("package.json"),
        r#"{"name":"query-wrapper-fixture"}"#,
    )
    .expect("write package json");
    std::fs::write(
        root.join("src/effect.ts"),
        "export const Fiber = {};\nexport const Queue = {};\nconst staleCache = 'refresh sqlite cache';\n",
    )
    .expect("write source");

    let output = asp_command(&root)
        .args([
            "rg",
            "-query",
            "Fiber|Queue",
            "-query",
            "stale|refresh|sqlite|cache",
            "--view",
            "graph-turbo-request",
            "src",
        ])
        .output()
        .expect("run asp rg repeated -query graph request");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("graph request json");
    assert_graph_turbo_request_contract(&payload);
    assert_eq!(
        payload["queryTerms"],
        serde_json::json!(["Fiber", "Queue", "stale", "refresh", "sqlite", "cache"]),
        "{payload}"
    );
    assert_eq!(
        payload["queryClauses"],
        serde_json::json!(["Fiber|Queue", "stale|refresh|sqlite|cache"]),
        "{payload}"
    );
    let actions = payload["actionFrontier"]
        .as_array()
        .expect("actionFrontier");
    assert_eq!(actions[0]["kind"], serde_json::json!("fd-query"));
    assert_eq!(
        actions[1]["kind"],
        serde_json::json!("multi-clause-rg-query")
    );
    assert_eq!(actions[1]["capabilityId"], serde_json::json!("rg"));
    assert_eq!(
        actions[1]["fields"]["queryClauses"],
        serde_json::json!(["Fiber|Queue", "stale|refresh|sqlite|cache"]),
        "{payload}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_rg_query_graph_request_includes_ablation_policy_from_env() {
    let root = temp_project_root("asp-rg-query-wrapper-ablation-policy");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn package_cohesion_seed() {}\npub fn query_clause_coverage() {}\n",
    )
    .expect("write source");

    let output = asp_command(&root)
        .env("ASP_GRAPH_TURBO_ABLATION_VARIANT", "no-package-cohesion")
        .args([
            "rg",
            "-query",
            "package cohesion",
            "--view",
            "graph-turbo-request",
            "src",
        ])
        .output()
        .expect("run asp rg ablation graph request");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("graph request json");
    assert_graph_turbo_request_contract(&payload);
    assert_eq!(
        payload["queryAdjustmentPolicy"],
        serde_json::json!({"packageCohesion": false}),
        "{payload}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_rg_query_graph_request_injects_package_path_runtime_seed_for_package_token() {
    let root = temp_project_root("asp-rg-query-wrapper-package-path-seed");
    std::fs::create_dir_all(root.join("packages/python/asp_graph_turbo/src/asp_graph_turbo"))
        .expect("create python package src");
    std::fs::create_dir_all(root.join("packages/python/asp_graph_turbo/tests"))
        .expect("create python package tests");
    std::fs::create_dir_all(root.join("crates/agent-semantic-client/tests/unit"))
        .expect("create rust tests");
    std::fs::write(root.join("pyproject.toml"), "[project]\nname='fixture'\n")
        .expect("write root pyproject");
    std::fs::write(
        root.join("packages/python/asp_graph_turbo/src/asp_graph_turbo/ranking_score.py"),
        "package scope runtime owner seed adjustment trace ablation metrics calibration\n",
    )
    .expect("write runtime python owner");
    std::fs::write(
        root.join("packages/python/asp_graph_turbo/tests/test_seed_prior.py"),
        "asp_graph_turbo P0 P1 P2 P3 P4 package scope test owner\n",
    )
    .expect("write python test owner");
    std::fs::write(
        root.join("crates/agent-semantic-client/tests/unit/search_history.rs"),
        "asp_graph_turbo P0 P1 P2 P3 P4 package scope owner\n",
    )
    .expect("write rust drift owner");

    let output = asp_command(&root)
        .args([
            "rg",
            "-query",
            "asp_graph_turbo P0 P1 P2 P3 P4",
            "-query",
            "package scope runtime owner seed adjustment trace ablation metrics calibration",
            "--view",
            "graph-turbo-request",
            ".",
        ])
        .output()
        .expect("run asp rg package path graph request");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("graph request json");
    assert_graph_turbo_request_contract(&payload);
    assert_eq!(
        payload["queryClauses"],
        serde_json::json!([
            "asp_graph_turbo P0 P1 P2 P3 P4",
            "package scope runtime owner seed adjustment trace ablation metrics calibration"
        ]),
        "{payload}"
    );
    assert!(
        payload["graph"]["nodes"]
            .as_array()
            .expect("graph nodes")
            .iter()
            .any(|node| node["id"].as_str()
                == Some(
                    "owner:packages/python/asp_graph_turbo/src/asp_graph_turbo/ranking_score.py"
                )),
        "{payload}"
    );
    let _ = std::fs::remove_dir_all(root);
}

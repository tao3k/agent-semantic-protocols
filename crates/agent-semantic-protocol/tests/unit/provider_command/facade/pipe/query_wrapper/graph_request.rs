use crate::provider_command::facade::pipe::assert_graph_turbo_request_contract;
use crate::provider_command::support::{asp_command, temp_project_root};

#[test]
fn asp_rg_query_graph_request_prefers_query_pack_after_repeated_query_clauses() {
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
    let actions = payload["actionFrontier"]
        .as_array()
        .expect("actionFrontier");
    assert_eq!(
        actions[0]["kind"],
        serde_json::json!("multi-clause-rg-query")
    );
    assert_eq!(actions[0]["capabilityId"], serde_json::json!("rg"));
    assert_eq!(
        actions[0]["fields"]["queryClauses"],
        serde_json::json!(["Fiber|Queue", "stale|refresh|sqlite|cache"]),
        "{payload}"
    );
    assert_eq!(actions[1]["kind"], serde_json::json!("fd-query"));
    let _ = std::fs::remove_dir_all(root);
}

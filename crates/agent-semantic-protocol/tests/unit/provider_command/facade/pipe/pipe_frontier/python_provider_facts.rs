use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation,
    write_provider_bin_config, write_semantic_facts_provider,
};
use serde_json::Value;

#[test]
fn search_pipe_graph_turbo_request_accepts_python_provider_semantic_facts() {
    let root = temp_project_root("search-pipe-python-provider-facts");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/model.py"),
        "class Bag:\n    items: list[str]\n",
    )
    .expect("write python source");
    write_semantic_facts_provider(
        &bin_dir,
        "py-harness",
        "[agent-semantic-client] syncing generated activation\n{\"schemaId\":\"agent.semantic-protocols.semantic-provider-facts\",\"schemaVersion\":\"1\",\"nodes\":[{\"id\":\"field:src/model.py-bag-items-2\",\"kind\":\"field\",\"role\":\"class-field\",\"value\":\"items: list[str]\",\"action\":\"code\",\"path\":\"src/model.py\",\"ownerPath\":\"src/model.py\",\"symbol\":\"items\",\"startLine\":2,\"endLine\":2,\"locator\":\"src/model.py:2:2\",\"matchText\":\"Bag.items: list[str]\",\"fields\":{\"containerName\":\"Bag\",\"fieldName\":\"items\",\"typeValue\":\"list[str]\",\"elementShape\":\"collection\",\"collectionKind\":\"list\",\"contextLocator\":\"src/model.py:1:2\"}}],\"edges\":[],\"candidateAnnotations\":[{\"path\":\"src/model.py\",\"attributes\":[\"generated\",\"schema-generated\"],\"source\":\"py-harness\",\"reason\":\"provider-parser-fact\"}]}\n",
        "",
    );
    write_provider_bin_config(&root, "python", &bin_dir.join("py-harness"));
    write_activation(&root, &[provider("python", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .env("ASP_PROVIDER_GRAPH_FACT_TIMEOUT_MS", "10000")
        .args([
            "python",
            "search",
            "pipe",
            "list|collection fields",
            "--view",
            "graph-turbo-request",
            ".",
        ])
        .output()
        .expect("run asp python search pipe graph request");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload: Value = serde_json::from_slice(&output.stdout).expect("graph request json");
    super::assert_graph_turbo_request_contract(&payload);
    let nodes = payload["graph"]["nodes"].as_array().expect("nodes");
    assert!(
        nodes.iter().any(|node| {
            node["kind"].as_str() == Some("field")
                && node["symbol"].as_str() == Some("items")
                && node["fields"]["collectionKind"].as_str() == Some("list")
        }),
        "{payload}"
    );
    assert!(
        nodes.iter().any(|node| {
            node["kind"].as_str() == Some("provider-candidate-annotation")
                && node["path"].as_str() == Some("src/model.py")
                && node["fields"]["attributes"][0].as_str() == Some("generated")
                && node["fields"]["attributes"][1].as_str() == Some("schema-generated")
        }),
        "{payload}"
    );
    let _ = std::fs::remove_dir_all(root);
}

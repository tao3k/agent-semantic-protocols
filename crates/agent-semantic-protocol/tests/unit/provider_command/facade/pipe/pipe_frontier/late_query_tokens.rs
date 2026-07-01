use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation,
    write_stdout_stderr_provider,
};
use serde_json::Value;

#[test]
fn search_pipe_graph_turbo_request_keeps_late_query_token_candidates() {
    let root = temp_project_root("search-pipe-graph-candidate-limit");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    let mut source = String::new();
    for index in 0..300 {
        source.push_str(&format!(
            "fn early_vec_{index}() {{ let _ = Vec::<u8>::new(); }}\n"
        ));
    }
    source.push_str("fn later_collection_fields() { let _ = \"collection fields\"; }\n");
    source
        .push_str("pub struct Scalar;\npub struct Snapshot {\n    pub scalars: Vec<Scalar>,\n}\n");
    std::fs::write(root.join("src/lib.rs"), source).expect("write source");
    write_stdout_stderr_provider(
        &bin_dir,
        "rs-harness",
        r#"{"nodes":[{"id":"field:src/lib.rs-scalars-304","kind":"field","role":"struct-field","value":"scalars: Vec<Scalar>","action":"code","path":"src/lib.rs","ownerPath":"src/lib.rs","symbol":"scalars","startLine":304,"endLine":304,"locator":"src/lib.rs:304:304","matchText":"Snapshot::scalars: Vec<Scalar>","fields":{"containerName":"Snapshot","fieldName":"scalars","typeName":"Vec","typeValue":"Vec<Scalar>","typeArgs":"Scalar","collectionKind":"Vec"}},{"id":"type:src/lib.rs-scalars-vec-304","kind":"type","role":"field-type","value":"Vec<Scalar>","action":"evidence","path":"src/lib.rs","ownerPath":"src/lib.rs","symbol":"Vec","startLine":304,"endLine":304,"locator":"src/lib.rs:304:304","fields":{"fieldName":"scalars","typeName":"Vec","typeValue":"Vec<Scalar>","typeArgs":"Scalar","collectionKind":"Vec"}},{"id":"collection:vec","kind":"collection","role":"family","value":"Vec","action":"evidence","symbol":"Vec","fields":{"collectionKind":"Vec"}}],"edges":[{"source":"field:src/lib.rs-scalars-304","target":"type:src/lib.rs-scalars-vec-304","relation":"has_type"},{"source":"field:src/lib.rs-scalars-304","target":"collection:vec","relation":"collection_of"}]}"#,
        "",
    );
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .env("ASP_PROVIDER_GRAPH_FACT_TIMEOUT_MS", "10000")
        .args([
            "rust",
            "search",
            "pipe",
            "Vec collection fields",
            "--view",
            "graph-turbo-request",
            ".",
        ])
        .output()
        .expect("run asp rust search pipe graph request");

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
            node["symbol"].as_str() == Some("collection")
                && node["matchText"]
                    .as_str()
                    .is_some_and(|text| text.contains("collection fields"))
        }),
        "{payload}"
    );
    assert!(
        nodes.iter().any(|node| {
            node["kind"].as_str() == Some("field")
                && node["symbol"].as_str() == Some("scalars")
                && node["fields"]["typeValue"].as_str() == Some("Vec<Scalar>")
        }),
        "{payload}"
    );
    assert!(
        nodes.iter().any(|node| {
            node["kind"].as_str() == Some("type")
                && node["fields"]["fieldName"].as_str() == Some("scalars")
                && node["fields"]["collectionKind"].as_str() == Some("Vec")
        }),
        "{payload}"
    );
    assert!(
        nodes.iter().any(|node| {
            node["kind"].as_str() == Some("collection") && node["symbol"].as_str() == Some("Vec")
        }),
        "{payload}"
    );
    let edges = payload["graph"]["edges"].as_array().expect("edges");
    assert!(
        edges
            .iter()
            .any(|edge| edge["relation"].as_str() == Some("has_type")),
        "{payload}"
    );
    assert!(
        edges
            .iter()
            .any(|edge| edge["relation"].as_str() == Some("collection_of")),
        "{payload}"
    );
    let _ = std::fs::remove_dir_all(root);
}

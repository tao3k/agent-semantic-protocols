use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation,
    write_stdout_stderr_provider,
};

use super::assert_graph_turbo_request_contract;
use serde_json::Value;

#[test]
fn search_pipe_graph_turbo_request_accepts_rust_provider_ontology_facts() {
    let root = temp_project_root("search-pipe-rust-provider-ontology-facts");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub struct Scalar;\npub struct Snapshot {\n    pub scalars: Vec<Scalar>,\n}\n",
    )
    .expect("write source");
    write_stdout_stderr_provider(
        &bin_dir,
        "rs-harness",
        r#"{"nodes":[{"id":"field:src/lib.rs-scalars-3","kind":"field","role":"struct-field","value":"scalars: Vec<Scalar>","action":"code","path":"src/lib.rs","ownerPath":"src/lib.rs","symbol":"scalars","startLine":3,"endLine":3,"locator":"src/lib.rs:1:4","matchText":"Snapshot::scalars: Vec<Scalar>","fields":{"languageId":"rust","providerId":"rs-harness","semanticFactKind":"field","provenance":"parser","confidence":"exact","freshness":"fresh","collectionFamily":"sequence","collectionImpl":"Vec","containerName":"Snapshot","fieldName":"scalars","typeName":"Vec","typeValue":"Vec<Scalar>","typeArgs":"Scalar","collectionKind":"Vec","elementShape":"scalar","contextLocator":"src/lib.rs:1:4","field":{"ownerKind":"struct","name":"scalars","ownerPath":"src/lib.rs","access":["read","append","validate"]}}},{"id":"type:src/lib.rs-scalars-vec-3","kind":"type","role":"field-type","value":"Vec<Scalar>","action":"evidence","path":"src/lib.rs","ownerPath":"src/lib.rs","symbol":"Vec","startLine":3,"endLine":3,"locator":"src/lib.rs:3:3","matchText":"Vec<Scalar>","fields":{"languageId":"rust","providerId":"rs-harness","semanticFactKind":"type","provenance":"parser","confidence":"exact","freshness":"fresh","collectionFamily":"sequence","collectionImpl":"Vec","fieldName":"scalars","typeName":"Vec","typeValue":"Vec<Scalar>","typeArgs":"Scalar","collectionKind":"Vec","type":{"name":"Vec<Scalar>","element":"Scalar"}}},{"id":"collection:vec","kind":"collection","role":"family","value":"Vec","action":"evidence","symbol":"Vec","fields":{"languageId":"rust","providerId":"rs-harness","semanticFactKind":"collection","provenance":"parser","confidence":"exact","freshness":"fresh","collectionFamily":"sequence","collectionImpl":"Vec","collectionKind":"Vec","collection":{"family":"sequence","impl":"Vec","elementType":"Scalar","mutation":["append","remove"]}}}],"edges":[{"source":"field:src/lib.rs-scalars-3","target":"type:src/lib.rs-scalars-vec-3","relation":"has_type"},{"source":"field:src/lib.rs-scalars-3","target":"collection:vec","relation":"collection_of"}]}"#,
        "",
    );
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "rust Vec collection fields",
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
    assert_graph_turbo_request_contract(&payload);
    let nodes = payload["graph"]["nodes"].as_array().expect("nodes");
    assert!(
        nodes.iter().any(|node| {
            node["kind"].as_str() == Some("field")
                && node["fields"]["semanticFactKind"].as_str() == Some("field")
                && node["fields"]["collectionFamily"].as_str() == Some("sequence")
                && node["fields"]["field"]["ownerKind"].as_str() == Some("struct")
                && node["fields"]["field"]["name"].as_str() == Some("scalars")
        }),
        "{payload}"
    );
    assert!(
        nodes.iter().any(|node| {
            node["kind"].as_str() == Some("type")
                && node["fields"]["semanticFactKind"].as_str() == Some("type")
                && node["fields"]["type"]["element"].as_str() == Some("Scalar")
        }),
        "{payload}"
    );
    assert!(
        nodes.iter().any(|node| {
            node["kind"].as_str() == Some("collection")
                && node["fields"]["semanticFactKind"].as_str() == Some("collection")
                && node["fields"]["collection"]["family"].as_str() == Some("sequence")
                && node["fields"]["collection"]["impl"].as_str() == Some("Vec")
        }),
        "{payload}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_graph_turbo_request_accepts_python_provider_ontology_facts() {
    let root = temp_project_root("search-pipe-python-provider-ontology-facts");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/model.py"),
        "class Bag:\n    items: list[str]\n",
    )
    .expect("write source");
    write_stdout_stderr_provider(
        &bin_dir,
        "py-harness",
        r#"{"nodes":[{"id":"field:src/model.py-bag-items-2","kind":"field","role":"class-field","value":"items: list[str]","action":"code","path":"src/model.py","ownerPath":"src/model.py","symbol":"items","startLine":2,"endLine":2,"locator":"src/model.py:1:2","matchText":"Bag.items: list[str]","fields":{"languageId":"python","providerId":"py-harness","semanticFactKind":"field","provenance":"parser","confidence":"exact","freshness":"fresh","collectionFamily":"sequence","collectionImpl":"list","containerName":"Bag","fieldName":"items","typeValue":"list[str]","elementShape":"collection","collectionKind":"list","contextLocator":"src/model.py:1:2","field":{"ownerKind":"class","name":"items","ownerPath":"src/model.py","access":["read","append","validate"]}}},{"id":"type:src/model.py-items-list-str-2","kind":"type","role":"field-type","value":"list[str]","action":"evidence","path":"src/model.py","ownerPath":"src/model.py","symbol":"list","startLine":2,"endLine":2,"locator":"src/model.py:2:2","fields":{"languageId":"python","providerId":"py-harness","semanticFactKind":"type","provenance":"parser","confidence":"exact","freshness":"fresh","collectionFamily":"sequence","collectionImpl":"list","fieldName":"items","typeValue":"list[str]","collectionKind":"list","type":{"name":"list[str]","element":"str"}}},{"id":"collection:list","kind":"collection","role":"family","value":"list","action":"evidence","symbol":"list","fields":{"languageId":"python","providerId":"py-harness","semanticFactKind":"collection","provenance":"parser","confidence":"exact","freshness":"fresh","collectionFamily":"sequence","collectionImpl":"list","collectionKind":"list","collection":{"family":"sequence","impl":"list","elementType":"str","mutation":["append","remove"]}}}],"edges":[{"source":"field:src/model.py-bag-items-2","target":"type:src/model.py-items-list-str-2","relation":"has_type"},{"source":"field:src/model.py-bag-items-2","target":"collection:list","relation":"collection_of"}]}"#,
        "",
    );
    write_activation(&root, &[provider("python", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "python",
            "search",
            "pipe",
            "python list collection fields",
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
    assert_graph_turbo_request_contract(&payload);
    let nodes = payload["graph"]["nodes"].as_array().expect("nodes");
    assert!(
        nodes.iter().any(|node| {
            node["kind"].as_str() == Some("field")
                && node["fields"]["languageId"].as_str() == Some("python")
                && node["fields"]["field"]["ownerKind"].as_str() == Some("class")
                && node["fields"]["field"]["name"].as_str() == Some("items")
        }),
        "{payload}"
    );
    assert!(
        nodes.iter().any(|node| {
            node["kind"].as_str() == Some("type")
                && node["fields"]["type"]["element"].as_str() == Some("str")
        }),
        "{payload}"
    );
    assert!(
        nodes.iter().any(|node| {
            node["kind"].as_str() == Some("collection")
                && node["fields"]["collection"]["family"].as_str() == Some("sequence")
                && node["fields"]["collection"]["impl"].as_str() == Some("list")
        }),
        "{payload}"
    );
    let _ = std::fs::remove_dir_all(root);
}

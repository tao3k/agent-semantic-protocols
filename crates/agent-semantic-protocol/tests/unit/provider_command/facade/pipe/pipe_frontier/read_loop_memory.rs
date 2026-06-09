use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation,
    write_stdout_stderr_provider,
};
use serde_json::Value;

#[test]
fn search_pipe_injects_read_loop_memory_into_graph_turbo_request_and_suppresses_seen_selector() {
    let root = temp_project_root("search-pipe-read-memory");
    let bin_dir = root.join(".bin");
    let package_root = root.join("languages/rust-harness");
    std::fs::create_dir_all(package_root.join("src")).expect("create package src");
    std::fs::write(
        package_root.join("src/lib.rs"),
        "pub struct Scalar;\npub struct Snapshot {\n    pub scalars: Vec<Scalar>,\n}\npub struct Other {\n    pub queued: Vec<Scalar>,\n}\n",
    )
    .expect("write package source");
    let memory_dir = root.join(".cache/agent-semantic-protocol");
    std::fs::create_dir_all(&memory_dir).expect("create read memory dir");
    std::fs::write(
        memory_dir.join("read-loop-memory.json"),
        r#"{"schemaId":"agent.semantic-protocols.read-loop-memory","schemaVersion":"1","projectRoot":".","seenSelectors":["languages/rust-harness/src/lib.rs:1:15"]}"#,
    )
    .expect("write read memory");
    write_stdout_stderr_provider(
        &bin_dir,
        "rs-harness",
        r#"{"nodes":[{"id":"field:src/lib.rs-scalars-3","kind":"field","role":"struct-field","value":"scalars: Vec<Scalar>","action":"code","path":"src/lib.rs","ownerPath":"src/lib.rs","symbol":"scalars","startLine":3,"endLine":3,"locator":"src/lib.rs:1:4","matchText":"Snapshot::scalars: Vec<Scalar>","fields":{"containerName":"Snapshot","fieldName":"scalars","typeValue":"Vec<Scalar>","collectionKind":"Vec","contextLocator":"src/lib.rs:1:4"}},{"id":"field:src/lib.rs-queued-6","kind":"field","role":"struct-field","value":"queued: Vec<Scalar>","action":"code","path":"src/lib.rs","ownerPath":"src/lib.rs","symbol":"queued","startLine":6,"endLine":6,"locator":"src/lib.rs:5:8","matchText":"Other::queued: Vec<Scalar>","fields":{"containerName":"Other","fieldName":"queued","typeValue":"Vec<Scalar>","collectionKind":"Vec","contextLocator":"src/lib.rs:5:8"}},{"id":"collection:vec","kind":"collection","role":"family","value":"Vec","action":"evidence","symbol":"Vec","fields":{"collectionKind":"Vec"}}],"edges":[{"source":"field:src/lib.rs-scalars-3","target":"collection:vec","relation":"collection_of"},{"source":"field:src/lib.rs-queued-6","target":"collection:vec","relation":"collection_of"}]}"#,
        "",
    );
    write_activation(&root, &[provider("rust", Vec::new())]);

    let request_output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "Vec collection fields",
            "--view",
            "graph-turbo-request",
            "languages/rust-harness",
        ])
        .output()
        .expect("run asp rust search pipe graph request with read memory");

    assert!(
        request_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&request_output.stderr)
    );
    let payload: Value =
        serde_json::from_slice(&request_output.stdout).expect("graph request json");
    assert_eq!(
        payload["readMemory"]["seenSelectors"][0], "languages/rust-harness/src/lib.rs:1:15",
        "{payload}"
    );

    let seeds_output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "Vec collection fields",
            "--view",
            "seeds",
            "languages/rust-harness",
        ])
        .output()
        .expect("run asp rust search pipe seeds with read memory");

    assert!(
        seeds_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&seeds_output.stderr)
    );
    let stdout = String::from_utf8(seeds_output.stdout).expect("stdout");
    assert!(stdout.contains("rankedEvidence="), "{stdout}");
    assert!(stdout.contains("evidenceFrontier="), "{stdout}");
    assert!(!stdout.contains("src/lib.rs:1:15"), "{stdout}");
    assert!(!stdout.contains("src/lib.rs:1:18"), "{stdout}");
    assert!(
        !stdout.contains(
            "frontierActions=S1.selector(selector=languages/rust-harness/src/lib.rs:1:15"
        ),
        "{stdout}"
    );
    assert!(!stdout.contains("frontierActions="), "{stdout}");
    assert!(
        stdout
            .contains("actionFrontier=A1.fd-query,A2.rg-query,A3.owner-items,A4.treesitter-query"),
        "{stdout}"
    );
    assert!(stdout.contains("recommendedNext=A1.fd-query"), "{stdout}");
    assert!(
        stdout.contains("nextCommand=asp fd -query Vec languages/rust-harness"),
        "{stdout}"
    );
    assert!(
        stdout.contains("reason=query-selector-low-confidence,owner-seed-base-required"),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}

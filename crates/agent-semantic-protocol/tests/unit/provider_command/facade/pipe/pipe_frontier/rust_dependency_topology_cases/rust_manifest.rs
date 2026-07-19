use crate::unit::provider_command::facade::pipe::pipe_frontier::rust_dependency_topology::support::{assert_manifest_dependency, rust_dependency_graph_request_payload, write_dependency_topology_provider};
use crate::provider_command::support;
#[test]
fn search_pipe_graph_request_uses_rust_manifest_dependency_versions() {
    let root = support::temp_project_root("search-pipe-rust-dependency-topology");
    let bin_dir = support::home_local_bin(&root);
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"dep-topology-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nserde = \"1\"\n",
    )
    .expect("write Cargo.toml");
    std::fs::write(
        root.join("Cargo.lock"),
        "[[package]]\nname = \"serde\"\nversion = \"1.0.228\"\n",
    )
    .expect("write Cargo.lock");
    std::fs::write(
        root.join("src/lib.rs"),
        "use serde::Serialize;\npub struct Receipt;\n",
    )
    .expect("write source");
    write_dependency_topology_provider(
        &bin_dir,
        "rs-harness",
        &marker,
        "serde",
        "1.0.228",
        "Cargo.toml",
    );
    support::write_activation(
        &root,
        &[support::provider_with_dependency_topology(
            "rust",
            Vec::new(),
        )],
    );

    let output = support::asp_command(&root)
        .env("PATH", support::prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "serde|Serialize",
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
    assert!(
        marker.exists(),
        "graph request should use provider-owned dependency topology"
    );
    assert_eq!(
        payload["cache"]["dependencySeed"]["topology"].as_str(),
        Some("provider-owned"),
        "{payload}"
    );
    let nodes = payload["graph"]["nodes"].as_array().expect("nodes");
    assert!(
        nodes.iter().any(|node| {
            node["kind"].as_str() == Some("dependency")
                && node["value"].as_str() == Some("serde")
                && node["confidence"].as_str() == Some("exact")
        }),
        "{payload}"
    );
    assert!(
        nodes.iter().any(|node| {
            node["kind"].as_str() == Some("dependency-version")
                && node["value"].as_str() == Some("serde@1.0.228")
        }),
        "{payload}"
    );
    let edges = payload["graph"]["edges"].as_array().expect("edges");
    assert!(
        edges
            .iter()
            .any(|edge| edge["relation"].as_str() == Some("version_locked")),
        "{payload}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_graph_request_reuses_cached_manifest_dependency_seed() {
    let root = support::temp_project_root("search-pipe-rust-dependency-seed-cache");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    let cache_home = root.join(".cache");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"dep-seed-cache-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nserde = \"1\"\n",
    )
    .expect("write Cargo.toml");
    std::fs::write(
        root.join("src/lib.rs"),
        "use serde::Serialize;\npub struct Receipt;\n",
    )
    .expect("write source");
    support::write_marker_provider(&bin_dir, "rs-harness", &marker);
    support::write_activation(&root, &[support::provider("rust", Vec::new())]);

    let first =
        rust_dependency_graph_request_payload(&root, &bin_dir, &cache_home, "serde|Serialize");
    assert_eq!(
        first["cache"]["dependencySeed"]["status"].as_str(),
        Some("miss"),
        "{first}"
    );
    assert_manifest_dependency(&first, "serde");

    let second =
        rust_dependency_graph_request_payload(&root, &bin_dir, &cache_home, "serde|Serialize");
    assert_eq!(
        second["cache"]["dependencySeed"]["status"].as_str(),
        Some("hit"),
        "{second}"
    );
    assert_manifest_dependency(&second, "serde");

    let _ = std::fs::remove_dir_all(root);
}

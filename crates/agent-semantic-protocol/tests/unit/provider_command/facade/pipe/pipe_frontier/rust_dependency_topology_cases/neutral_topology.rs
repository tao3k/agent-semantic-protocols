use crate::provider_command::support;
use crate::unit::provider_command::facade::pipe::pipe_frontier::rust_dependency_topology::support::assert_manifest_dependency;
#[test]
fn search_pipe_graph_request_includes_language_neutral_project_topology() {
    let root = support::temp_project_root("search-pipe-project-topology");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::create_dir_all(root.join("zz-harnesses/gerbil-scheme-language-project-harness/src"))
        .expect("create submodule path");
    std::fs::write(
        root.join(".gitmodules"),
        "[submodule \"zz-harnesses/gerbil-scheme-language-project-harness\"]\n\
         \tpath = zz-harnesses/gerbil-scheme-language-project-harness\n\
         \turl = https://example.invalid/gerbil.git\n",
    )
    .expect("write .gitmodules");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"project-topology-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write Cargo.toml");
    std::fs::write(
        root.join("zz-harnesses/gerbil-scheme-language-project-harness/Cargo.toml"),
        "[package]\nname = \"submodule-topology-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write submodule Cargo.toml");
    std::fs::write(root.join("src/lib.rs"), "pub struct TopologyReceipt;\n").expect("write source");
    std::fs::write(
        root.join("src/submodule_topology.rs"),
        "pub struct SubmoduleTopologyReceipt;\n",
    )
    .expect("write root topology source");
    std::fs::write(
        root.join("zz-harnesses/gerbil-scheme-language-project-harness/src/lib.rs"),
        "pub struct SubmoduleTopologyReceipt;\n",
    )
    .expect("write submodule source");
    support::write_marker_provider(&bin_dir, "rs-harness", &marker);
    support::write_activation(&root, &[support::provider("rust", Vec::new())]);

    let output = support::asp_command(&root)
        .env("PATH", support::prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "SubmoduleTopologyReceipt|topology",
            "--view",
            "graph-turbo-request",
            ".",
        ])
        .output()
        .expect("run asp rust search pipe topology graph request");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload: Value = serde_json::from_slice(&output.stdout).expect("graph request json");
    assert_graph_turbo_request_contract(&payload);
    assert!(
        payload["surfaces"]
            .as_array()
            .expect("surfaces")
            .iter()
            .any(|surface| surface.as_str() == Some("topology")),
        "{payload}"
    );
    assert_eq!(
        payload["fields"]["topologyRank"].as_str(),
        Some("submodule-membership"),
        "{payload}"
    );
    assert_eq!(
        payload["summary"]["topologyRankSubmodules"].as_u64(),
        Some(1),
        "{payload}"
    );
    let nodes = payload["graph"]["nodes"].as_array().expect("nodes");
    assert!(
        nodes.iter().any(|node| {
            node["kind"].as_str() == Some("workspace")
                && node["role"].as_str() == Some("root")
                && node["value"].as_str() == Some(".")
        }),
        "{payload}"
    );
    assert!(
        nodes.iter().any(|node| {
            node["kind"].as_str() == Some("provider-root")
                && node["role"].as_str() == Some("language-root")
                && node["fields"]["languageId"].as_str() == Some("rust")
        }),
        "{payload}"
    );
    let submodule_id = "submodule:zz-harnesses/gerbil-scheme-language-project-harness";
    assert!(
        nodes.iter().any(|node| {
            node["id"].as_str() == Some(submodule_id)
                && node["kind"].as_str() == Some("submodule")
                && node["role"].as_str() == Some("workspace-member")
                && node["value"].as_str()
                    == Some("zz-harnesses/gerbil-scheme-language-project-harness")
        }),
        "{payload}"
    );
    let root_project_id = "language-project:rust-.";
    let root_config_id = "project-marker:rust-cargo.toml";
    let root_dependency_marker_id = "dependency-marker:rust-cargo.toml";
    let submodule_project_id =
        "language-project:rust-zz-harnesses/gerbil-scheme-language-project-harness";
    let submodule_config_id =
        "project-marker:rust-zz-harnesses/gerbil-scheme-language-project-harness/cargo.toml";
    assert!(
        nodes.iter().any(|node| {
            node["id"].as_str() == Some(root_project_id)
                && node["kind"].as_str() == Some("language-project")
                && node["role"].as_str() == Some("project-root")
                && node["fields"]["languageId"].as_str() == Some("rust")
                && node["fields"]["projectMarker"].as_str() == Some("Cargo.toml")
        }),
        "{payload}"
    );
    assert!(
        nodes.iter().any(|node| {
            node["id"].as_str() == Some(submodule_project_id)
                && node["kind"].as_str() == Some("language-project")
                && node["role"].as_str() == Some("project-root")
                && node["path"].as_str()
                    == Some("zz-harnesses/gerbil-scheme-language-project-harness")
                && node["fields"]["projectMarker"].as_str()
                    == Some("zz-harnesses/gerbil-scheme-language-project-harness/Cargo.toml")
        }),
        "{payload}"
    );
    assert!(
        nodes.iter().any(|node| {
            node["id"].as_str() == Some(root_config_id)
                && node["kind"].as_str() == Some("project-marker")
                && node["role"].as_str() == Some("project-marker")
        }),
        "{payload}"
    );
    assert!(
        nodes.iter().any(|node| {
            node["id"].as_str() == Some(root_dependency_marker_id)
                && node["kind"].as_str() == Some("dependency-marker")
                && node["role"].as_str() == Some("dependency-source")
        }),
        "{payload}"
    );
    assert!(
        nodes.iter().any(|node| {
            node["id"].as_str() == Some(submodule_config_id)
                && node["kind"].as_str() == Some("project-marker")
                && node["role"].as_str() == Some("project-marker")
        }),
        "{payload}"
    );
    let edges = payload["graph"]["edges"].as_array().expect("edges");
    let root_owner_id = "owner:src/submodule_topology.rs";
    let owner_id = "owner:zz-harnesses/gerbil-scheme-language-project-harness/src/lib.rs";
    let root_owner_index = nodes
        .iter()
        .position(|node| node["id"].as_str() == Some(root_owner_id))
        .expect("root owner node");
    let submodule_owner_index = nodes
        .iter()
        .position(|node| node["id"].as_str() == Some(owner_id))
        .expect("submodule owner node");
    assert!(
        submodule_owner_index < root_owner_index,
        "submodule topology owner should outrank matching root owner: {payload}"
    );

    let disabled_output = support::asp_command(&root)
        .env("PATH", support::prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .env("ASP_GRAPH_TURBO_ABLATION_VARIANT", "no-topology-membership")
        .args([
            "rust",
            "search",
            "pipe",
            "SubmoduleTopologyReceipt|topology",
            "--view",
            "graph-turbo-request",
            ".",
        ])
        .output()
        .expect("run asp rust search pipe topology graph request without topology ranking");

    assert!(
        disabled_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&disabled_output.stderr)
    );
    let disabled_payload: Value =
        serde_json::from_slice(&disabled_output.stdout).expect("disabled graph request json");
    assert_graph_turbo_request_contract(&disabled_payload);
    assert_eq!(
        disabled_payload["queryAdjustmentPolicy"]["topologyMembership"].as_bool(),
        Some(false),
        "{disabled_payload}"
    );
    assert!(
        disabled_payload["fields"]["topologyRank"].is_null(),
        "disabled topology membership must not claim topology rank signal: {disabled_payload}"
    );
    assert_eq!(
        disabled_payload["summary"]["topologyRankSubmodules"].as_u64(),
        Some(1),
        "{disabled_payload}"
    );
    let disabled_nodes = disabled_payload["graph"]["nodes"]
        .as_array()
        .expect("nodes");
    let disabled_root_owner_index = disabled_nodes
        .iter()
        .position(|node| node["id"].as_str() == Some(root_owner_id))
        .expect("disabled root owner node");
    let disabled_submodule_owner_index = disabled_nodes
        .iter()
        .position(|node| node["id"].as_str() == Some(owner_id))
        .expect("disabled submodule owner node");
    assert!(
        disabled_root_owner_index < disabled_submodule_owner_index,
        "disabling topology membership should expose the baseline owner order: {disabled_payload}"
    );
    assert!(
        edges.iter().any(|edge| {
            edge["relation"].as_str() == Some("has_provider_root")
                && edge["source"].as_str() == Some("workspace:.")
        }),
        "{payload}"
    );
    assert!(
        edges.iter().any(|edge| {
            edge["relation"].as_str() == Some("has_submodule")
                && edge["source"].as_str() == Some("workspace:.")
                && edge["target"].as_str() == Some(submodule_id)
        }),
        "{payload}"
    );
    assert!(
        edges.iter().any(|edge| {
            edge["relation"].as_str() == Some("has_language_project")
                && edge["source"].as_str() == Some("provider-root:rust-.")
                && edge["target"].as_str() == Some(root_project_id)
        }),
        "{payload}"
    );
    assert!(
        edges.iter().any(|edge| {
            edge["relation"].as_str() == Some("declared_by")
                && edge["source"].as_str() == Some(root_project_id)
                && edge["target"].as_str() == Some(root_config_id)
        }),
        "{payload}"
    );
    assert!(
        edges.iter().any(|edge| {
            edge["relation"].as_str() == Some("uses_dependency_marker")
                && edge["source"].as_str() == Some(root_project_id)
                && edge["target"].as_str() == Some(root_dependency_marker_id)
        }),
        "{payload}"
    );
    assert!(
        edges.iter().any(|edge| {
            edge["relation"].as_str() == Some("contains_project")
                && edge["source"].as_str() == Some(submodule_id)
                && edge["target"].as_str() == Some(submodule_project_id)
        }),
        "{payload}"
    );
    assert!(
        edges.iter().any(|edge| {
            edge["relation"].as_str() == Some("contains")
                && edge["source"].as_str() == Some(submodule_id)
                && edge["target"].as_str() == Some(owner_id)
        }),
        "{payload}"
    );
    let _ = std::fs::remove_dir_all(root);
}

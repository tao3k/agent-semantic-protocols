use std::fs;

use crate::graph_candidate_projection::GraphProjectionCandidate;
use crate::graph_topology_projection::{
    GraphTopologyProjectionRequest, graph_path_is_under,
    graph_project_submodule_paths_from_content, graph_project_topology_projection,
    graph_submodule_owner_edges,
};

#[test]
fn graph_topology_projection_discovers_project_and_dependency_markers() {
    let root = tempfile::Builder::new()
        .prefix("asp-graph-topology-projection-")
        .tempdir()
        .expect("tempdir");
    fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = \"topology-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[dependencies]\nserde = \"1\"\n",
    )
    .expect("write cargo manifest");
    fs::write(root.path().join("Cargo.lock"), "# lock\n").expect("write cargo lock");
    fs::create_dir_all(root.path().join("src")).expect("create src");
    fs::write(
        root.path().join("src/lib.rs"),
        "pub fn topology_fixture() {}\n",
    )
    .expect("write source");

    let candidates = vec![GraphProjectionCandidate::new(
        "src/lib.rs",
        1,
        1,
        "topology_fixture",
        "pub fn topology_fixture() {}",
        "source-index",
        "high",
    )];
    let projection = graph_project_topology_projection(GraphTopologyProjectionRequest::new(
        "rust",
        root.path(),
        &candidates,
    ));

    assert!(
        projection
            .nodes
            .iter()
            .any(|node| node["kind"] == "workspace"),
        "{projection:?}"
    );
    assert!(
        projection
            .nodes
            .iter()
            .any(|node| { node["kind"] == "language-project" && node["path"] == "." })
    );
    assert!(
        projection
            .nodes
            .iter()
            .any(|node| { node["kind"] == "project-marker" && node["path"] == "Cargo.toml" })
    );
    assert!(
        projection
            .nodes
            .iter()
            .any(|node| { node["kind"] == "dependency-marker" && node["path"] == "Cargo.lock" })
    );
    assert!(
        projection
            .edges
            .iter()
            .any(|edge| { edge["relation"] == "has_language_project" })
    );
}

#[test]
fn graph_topology_projection_projects_submodule_owner_edges() {
    let root = tempfile::Builder::new()
        .prefix("asp-graph-submodule-projection-")
        .tempdir()
        .expect("tempdir");
    fs::write(
        root.path().join(".gitmodules"),
        "[submodule \"languages/rust\"]\n  path = languages/rust\n  url = https://example.invalid/rust.git\n",
    )
    .expect("write gitmodules");

    let owners = vec![
        "languages/rust/src/lib.rs".to_string(),
        "src/lib.rs".to_string(),
    ];
    let edges = graph_submodule_owner_edges(root.path(), &owners);

    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0]["relation"], "contains");
    assert_eq!(edges[0]["source"], "submodule:languages/rust");
    assert_eq!(edges[0]["target"], "owner:languages/rust/src/lib.rs");
}

#[test]
fn graph_topology_submodule_paths_are_normalized_and_relative() {
    let paths = graph_project_submodule_paths_from_content(
        "path = languages/rust\npath = /absolute\npath = languages\\python\n",
    );

    assert_eq!(
        paths,
        vec!["languages/python".to_string(), "languages/rust".to_string()]
    );
    assert!(graph_path_is_under(
        "languages/rust/src/lib.rs",
        "languages/rust"
    ));
    assert!(!graph_path_is_under(
        "languages/rusty/src/lib.rs",
        "languages/rust"
    ));
}

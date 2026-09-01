use std::fs;

use crate::graph_candidate_projection::GraphProjectionCandidate;
use crate::graph_topology_projection::{
    GraphOwnerMissingTopologyRequest, GraphTopologyProjectionRequest,
    graph_owner_missing_topology_projection, graph_path_is_under,
    graph_project_submodule_paths_from_content, graph_project_topology_projection,
    graph_submodule_owner_edges,
};

#[test]
fn graph_topology_projection_does_not_infer_package_graph_from_marker_files() {
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
        &"rust".into(),
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
    assert!(projection.nodes.iter().all(|node| !matches!(
        node["kind"].as_str(),
        Some("language-project" | "project-marker" | "dependency-marker")
    )));
    assert!(
        projection
            .edges
            .iter()
            .all(|edge| edge["relation"] != "has_language_project")
    );
}

#[test]
fn owner_missing_topology_is_bounded_and_explains_provider_scope_expectation() {
    let admitted = admitted_rust_project_resolution();
    let projection = graph_owner_missing_topology_projection(GraphOwnerMissingTopologyRequest {
        language_id: "rust",
        owner_path: "crates/demo/src/lib.rs",
        generation_digest: "generation-42",
        root_digest: "root-42",
        project_resolutions: std::slice::from_ref(&admitted),
    });

    assert!(projection.nodes.len() <= 10, "{projection:?}");
    assert!(projection.edges.len() <= 12, "{projection:?}");
    for relation in [
        "has_provider_root",
        "omits_owner",
        "has_language_project",
        "resolves_source_scope",
        "expects_owner",
    ] {
        assert!(
            projection
                .edges
                .iter()
                .any(|edge| edge["relation"] == relation),
            "missing {relation}: {projection:?}"
        );
    }
}

#[test]
fn graph_topology_projection_uses_admitted_provider_project_resolution() {
    let root = tempfile::Builder::new()
        .prefix("asp-admitted-project-topology-")
        .tempdir()
        .expect("tempdir");
    let candidates = vec![GraphProjectionCandidate::new(
        "crates/demo/src/lib.rs",
        1,
        1,
        "demo",
        "pub fn demo() {}",
        "native-owner",
        "high",
    )];
    let admitted = admitted_rust_project_resolution();
    let language_id = "rust".into();
    let projection = graph_project_topology_projection(
        GraphTopologyProjectionRequest::new(&language_id, root.path(), &candidates)
            .with_project_resolutions(std::slice::from_ref(&admitted)),
    );

    for kind in [
        "workspace",
        "provider-root",
        "language-project",
        "project-marker",
        "dependency-marker",
        "package",
        "target",
        "source-scope",
    ] {
        assert!(
            projection.nodes.iter().any(|node| node["kind"] == kind),
            "missing {kind}: {projection:?}"
        );
    }
    for relation in [
        "has_provider_root",
        "has_language_project",
        "declared_by",
        "uses_dependency_marker",
        "contains_package",
        "declares_target",
        "resolves_source_scope",
        "admits_owner",
    ] {
        assert!(
            projection
                .edges
                .iter()
                .any(|edge| edge["relation"] == relation),
            "missing {relation}: {projection:?}"
        );
    }
    let owner_edge = projection
        .edges
        .iter()
        .find(|edge| edge["relation"] == "admits_owner")
        .expect("source scope admits native owner candidate");
    assert_eq!(owner_edge["target"], "owner:crates/demo/src/lib.rs");
}

#[test]
fn graph_topology_projection_does_not_admit_candidate_outside_provider_scope() {
    let root = tempfile::Builder::new()
        .prefix("asp-project-topology-outside-scope-")
        .tempdir()
        .expect("tempdir");
    let candidates = vec![GraphProjectionCandidate::new(
        "scripts/unscoped.rs",
        1,
        1,
        "unscoped",
        "fn unscoped() {}",
        "native-owner",
        "high",
    )];
    let admitted = admitted_rust_project_resolution();
    let language_id = "rust".into();
    let projection = graph_project_topology_projection(
        GraphTopologyProjectionRequest::new(&language_id, root.path(), &candidates)
            .with_project_resolutions(std::slice::from_ref(&admitted)),
    );

    assert!(
        projection
            .edges
            .iter()
            .all(|edge| edge["relation"] != "admits_owner"),
        "provider scope must own topology membership: {projection:?}"
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

fn admitted_rust_project_resolution() -> agent_semantic_content_identity::AdmittedProjectResolution
{
    let resolution = agent_semantic_content_identity::ProjectResolutionReceipt {
        schema_id: "agent.semantic-protocols.project-resolution".to_owned(),
        schema_version: "1".to_owned(),
        state: "resolved".to_owned(),
        completeness: "exact".to_owned(),
        language_id: "rust".to_owned(),
        provider_id: "asp-rust".to_owned(),
        parser_id: "cargo-project-resolution-v1".to_owned(),
        candidate_generation_digest: "candidate-generation".to_owned(),
        project_entry: "Cargo.toml".to_owned(),
        package_graph: agent_semantic_content_identity::LanguagePackageGraph {
            schema_id: "agent.semantic-protocols.language-package-graph".to_owned(),
            schema_version: "1".to_owned(),
            language_id: "rust".to_owned(),
            provider_id: "asp-rust".to_owned(),
            project_entry: "Cargo.toml".to_owned(),
            parser_id: "cargo-project-resolution-v1".to_owned(),
            manifests: vec![agent_semantic_content_identity::ProjectFile {
                path: "Cargo.toml".to_owned(),
                kind: "cargo-manifest".to_owned(),
                digest: "manifest-digest".to_owned(),
            }],
            lockfiles: vec![agent_semantic_content_identity::ProjectFile {
                path: "Cargo.lock".to_owned(),
                kind: "cargo-lockfile".to_owned(),
                digest: "lock-digest".to_owned(),
            }],
            packages: vec![agent_semantic_content_identity::LanguagePackage {
                package_id: "demo".to_owned(),
                name: "demo".to_owned(),
                version: Some("0.1.0".to_owned()),
                manifest_path: "Cargo.toml".to_owned(),
                root: ".".to_owned(),
                workspace_member: true,
                targets: vec![agent_semantic_content_identity::LanguageTarget {
                    target_id: "demo-lib".to_owned(),
                    kind: "lib".to_owned(),
                    name: "demo".to_owned(),
                    explicit: false,
                    source_roots: vec!["src".to_owned()],
                    entrypoints: vec!["src/lib.rs".to_owned()],
                    generated_roots: Vec::new(),
                }],
            }],
            internal_dependency_edges: Vec::new(),
            external_dependencies: Vec::new(),
            unresolved: Vec::new(),
        },
        source_scopes: vec![agent_semantic_content_identity::ResolvedSourceScope {
            schema_id: None,
            schema_version: None,
            scope_id: "demo-lib-scope".to_owned(),
            package_id: "demo".to_owned(),
            target_id: "demo-lib".to_owned(),
            roots: vec!["src".to_owned()],
            explicit_paths: vec!["src/lib.rs".to_owned()],
            extensions: vec![".rs".to_owned()],
            include_authority: "cargo-target".to_owned(),
            classifications: vec!["library".to_owned()],
            exclusions: Vec::new(),
            provider_facts: None,
            conflicts: Vec::new(),
            resolution_state: Some("resolved".to_owned()),
            scope_digest: Some("scope-digest".to_owned()),
        }],
        conflicts: Vec::new(),
        metrics: agent_semantic_content_identity::ProjectResolutionMetrics {
            parsed_manifest_count: 1,
            parsed_lockfile_count: 1,
            affected_package_count: 1,
            full_workspace_reads: 0,
            full_manifest_reparses: 0,
            db_opens: 0,
            elapsed_micros: 1,
        },
    };
    agent_semantic_content_identity::AdmittedProjectResolution::new("crates/demo", resolution)
        .expect("admit project resolution")
}

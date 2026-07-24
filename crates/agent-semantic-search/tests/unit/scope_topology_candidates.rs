use crate::{
    SEARCH_PIPE_SCOPE_TOPOLOGY_SOURCE, SearchPipeScopeTopologyAcquisitionRequest,
    SemanticWorkspaceScope, collect_search_pipe_scope_topology_acquisition,
};
use serde_json::json;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn workspace_scope_topology_projects_only_bounded_admitted_source_files() {
    let root = temporary_root("scope-topology-admission");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::create_dir_all(root.join("ignored")).expect("create ignored");
    std::fs::write(root.join("scope.toml"), "[scope]\n").expect("write manifest");
    std::fs::write(root.join("src/a.py"), "class A: pass\n").expect("write a.py");
    std::fs::write(root.join("src/b.py"), "class B: pass\n").expect("write b.py");
    std::fs::write(root.join("src/not_python.rs"), "struct Rust;\n").expect("write rust");
    std::fs::write(root.join("ignored/hidden.py"), "class Hidden: pass\n")
        .expect("write ignored python");
    let scope = scope(&root);

    let acquisition =
        collect_search_pipe_scope_topology_acquisition(SearchPipeScopeTopologyAcquisitionRequest {
            workspace_scope: &scope,
            locator_root: &root,
            ignore_dirs: &["ignored".to_string()],
            include_hidden_dirs: &[],
            entry_visit_limit: 256,
            candidate_limit: 12,
        })
        .expect("collect scope topology");

    let paths = acquisition
        .candidates
        .iter()
        .map(|candidate| candidate.path.as_str())
        .collect::<Vec<_>>();
    assert_eq!(paths, ["src/a.py", "src/b.py"]);
    assert_eq!(
        acquisition.candidate_sources,
        [SEARCH_PIPE_SCOPE_TOPOLOGY_SOURCE]
    );
    assert_eq!(acquisition.source_trace[0].status, "used");
    assert_eq!(acquisition.source_trace[0].matched, 2);
    assert!(!paths.contains(&"ignored/hidden.py"));
    assert!(!paths.contains(&"src/not_python.rs"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn workspace_scope_topology_stops_at_the_hard_entry_budget() {
    let root = temporary_root("scope-topology-hard-budget");
    std::fs::write(root.join("scope.toml"), "[scope]\n").expect("write manifest");
    std::fs::write(root.join("a.py"), "class A: pass\n").expect("write a.py");
    std::fs::write(root.join("b.py"), "class B: pass\n").expect("write b.py");
    let scope = scope(&root);

    let acquisition =
        collect_search_pipe_scope_topology_acquisition(SearchPipeScopeTopologyAcquisitionRequest {
            workspace_scope: &scope,
            locator_root: &root,
            ignore_dirs: &[],
            include_hidden_dirs: &[],
            entry_visit_limit: 1,
            candidate_limit: 12,
        })
        .expect("collect bounded scope topology");

    assert_eq!(acquisition.source_trace[0].status, "truncated");
    assert_eq!(acquisition.source_trace[0].normalized, 1);
    assert!(acquisition.candidates.len() <= 1);
    let _ = std::fs::remove_dir_all(root);
}

fn scope(root: &Path) -> SemanticWorkspaceScope {
    SemanticWorkspaceScope::from_packet(&json!({
        "schemaId": "agent.semantic-protocols.semantic-workspace-scope",
        "schemaVersion": "1",
        "workspaceId": "python:test",
        "languageId": "python",
        "providerId": "py-harness",
        "packageManager": "test",
        "sourceExtensions": [".py"],
        "discoveryRoot": root,
        "anchors": [{
            "kind": "test-manifest",
            "path": root.join("scope.toml"),
            "sha256": format!("sha256:{}", "0".repeat(64))
        }],
        "packages": [{
            "packageId": "python:test",
            "name": "test",
            "root": root,
            "manifestPath": root.join("scope.toml"),
            "languageId": "python"
        }],
        "admittedRoots": [root],
        "fingerprint": format!("sha256:{}", "1".repeat(64))
    }))
    .expect("parse workspace scope")
}

fn temporary_root(label: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-semantic-search-{label}-{nonce}"));
    std::fs::create_dir_all(&root).expect("create temporary root");
    root
}

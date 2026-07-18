use std::path::Path;

use serde_json::json;

use crate::{SemanticWorkspaceScope, SemanticWorkspaceScopeSet};

fn packet() -> serde_json::Value {
    json!({
        "schemaId": "agent.semantic-protocols.semantic-workspace-scope",
        "schemaVersion": "1",
        "workspaceId": "python:root",
        "languageId": "python",
        "providerId": "py-harness",
        "packageManager": "uv",
        "sourceExtensions": [".py", ".pyi"],
        "discoveryRoot": "/work/root",
        "anchors": [{
            "kind": "pyproject",
            "path": "/work/root/pyproject.toml",
            "sha256": format!("sha256:{}", "b".repeat(64))
        }],
        "packages": [
            {
                "packageId": "python:root",
                "name": "root",
                "root": "/work/root",
                "manifestPath": "/work/root/pyproject.toml",
                "languageId": "python"
            },
            {
                "packageId": "python:shared",
                "name": "shared",
                "root": "/work/shared",
                "manifestPath": "/work/shared/pyproject.toml",
                "languageId": "python"
            }
        ],
        "admittedRoots": ["/work/root", "/work/shared"],
        "fingerprint": format!("sha256:{}", "a".repeat(64))
    })
}

#[test]
fn admits_relative_and_external_workspace_members() {
    let scope = SemanticWorkspaceScope::from_packet(&packet()).expect("valid scope");

    let local = scope
        .admit_candidate(Path::new("src/app.py"), "python")
        .expect("local candidate");
    assert_eq!(local.package_id, "python:root");
    assert_eq!(local.canonical_path, Path::new("/work/root/src/app.py"));

    let external = scope
        .admit_candidate(Path::new("/work/shared/src/api.py"), "python")
        .expect("external workspace member");
    assert_eq!(external.package_id, "python:shared");
}

#[test]
fn rejects_parent_repository_and_language_drift_before_rank() {
    let scope = SemanticWorkspaceScope::from_packet(&packet()).expect("valid scope");

    let outside = scope
        .admit_candidate(Path::new("/work/crates/unrelated.rs"), "python")
        .expect_err("outside candidate");
    assert_eq!(outside.reason_kind, "candidate-out-of-scope");

    let language = scope
        .admit_candidate(Path::new("src/app.py"), "rust")
        .expect_err("language drift");
    assert_eq!(language.reason_kind, "candidate-language-mismatch");

    let extension = scope
        .admit_candidate(Path::new("src/app.rs"), "python")
        .expect_err("provider-owned source extension drift");
    assert_eq!(extension.reason_kind, "candidate-language-mismatch");

    let anchor = scope
        .admit_candidate(Path::new("pyproject.toml"), "python")
        .expect("provider anchor remains admissible");
    assert_eq!(anchor.package_id, "python:root");
}

#[test]
fn resolves_repository_relative_candidates_before_scope_admission() {
    let scope = SemanticWorkspaceScope::from_packet(&packet()).expect("valid scope");

    let admitted = scope
        .admit_candidate_from(Path::new("/work"), Path::new("root/src/app.py"), "python")
        .expect("repository-relative Python owner");
    assert_eq!(admitted.package_id, "python:root");

    let rejected = scope
        .admit_candidate_from(
            Path::new("/work"),
            Path::new("crates/unrelated.rs"),
            "python",
        )
        .expect_err("repository-relative Rust owner");
    assert_eq!(rejected.reason_kind, "candidate-out-of-scope");
}

#[test]
fn rejects_relative_provider_roots() {
    let mut value = packet();
    value["packages"][0]["root"] = json!(".");
    assert!(SemanticWorkspaceScope::from_packet(&value).is_err());
}

#[test]
fn rejects_packets_that_only_look_admissible_but_violate_scope_contract() {
    let mut missing_anchors = packet();
    missing_anchors["anchors"] = json!([]);
    assert!(SemanticWorkspaceScope::from_packet(&missing_anchors).is_err());

    let mut relative_manifest = packet();
    relative_manifest["packages"][0]["manifestPath"] = json!("pyproject.toml");
    assert!(SemanticWorkspaceScope::from_packet(&relative_manifest).is_err());

    let mut invalid_fingerprint = packet();
    invalid_fingerprint["fingerprint"] = json!("sha256:not-a-digest");
    assert!(SemanticWorkspaceScope::from_packet(&invalid_fingerprint).is_err());

    let mut missing_extensions = packet();
    missing_extensions["sourceExtensions"] = json!([]);
    assert!(SemanticWorkspaceScope::from_packet(&missing_extensions).is_err());
}

#[test]
fn accepts_provider_owned_package_manager_and_anchor_kinds() {
    let mut value = packet();
    value["languageId"] = json!("future-lang");
    value["packageManager"] = json!("future-pm");
    value["anchors"][0]["kind"] = json!("future-workspace-anchor");
    for package in value["packages"].as_array_mut().expect("packages") {
        package["languageId"] = json!("future-lang");
    }

    let scope = SemanticWorkspaceScope::from_packet(&value).expect("provider-owned kinds");
    assert_eq!(scope.language_id, "future-lang");
    assert_eq!(scope.package_manager, "future-pm");
    assert_eq!(scope.anchors[0].kind, "future-workspace-anchor");
}

#[test]
fn admits_virtual_workspace_anchor_with_workspace_identity() {
    let mut value = packet();
    value["workspaceId"] = json!("python:virtual");
    value["discoveryRoot"] = json!("/work/virtual");
    value["anchors"][0]["path"] = json!("/work/virtual/pyproject.toml");
    value["packages"] = json!([{
        "packageId": "python:shared",
        "name": "shared",
        "root": "/work/shared",
        "manifestPath": "/work/shared/pyproject.toml",
        "languageId": "python"
    }]);
    value["admittedRoots"] = json!(["/work/shared"]);

    let scope = SemanticWorkspaceScope::from_packet(&value).expect("virtual workspace scope");
    let admission = scope
        .admit_candidate(Path::new("pyproject.toml"), "python")
        .expect("virtual workspace anchor");
    assert_eq!(admission.package_id, "python:virtual");
}

#[test]
fn scope_set_routes_candidates_from_provider_owned_extensions() {
    let python = SemanticWorkspaceScope::from_packet(&packet()).expect("python scope");
    let mut rust_packet = packet();
    rust_packet["workspaceId"] = json!("rust:root");
    rust_packet["languageId"] = json!("rust");
    rust_packet["providerId"] = json!("rs-harness");
    rust_packet["packageManager"] = json!("cargo");
    rust_packet["sourceExtensions"] = json!([".rs"]);
    rust_packet["anchors"] = json!([{
        "kind": "cargo-manifest",
        "path": "/work/root/Cargo.toml",
        "sha256": format!("sha256:{}", "c".repeat(64))
    }]);
    for package in rust_packet["packages"].as_array_mut().expect("packages") {
        let name = package["name"].as_str().expect("name").to_owned();
        let root = package["root"].as_str().expect("root").to_owned();
        package["packageId"] = json!(format!("rust:{name}"));
        package["manifestPath"] = json!(format!("{root}/Cargo.toml"));
        package["languageId"] = json!("rust");
    }
    let rust = SemanticWorkspaceScope::from_packet(&rust_packet).expect("rust scope");
    let scopes = SemanticWorkspaceScopeSet::new(vec![python, rust]).expect("scope set");

    let python_admission = scopes
        .admit_candidate_from(Path::new("/work"), Path::new("root/src/app.py"))
        .expect("Python admission");
    assert_eq!(python_admission.provider_id, "py-harness");
    let rust_admission = scopes
        .admit_candidate_from(Path::new("/work"), Path::new("root/src/lib.rs"))
        .expect("Rust admission");
    assert_eq!(rust_admission.provider_id, "rs-harness");
}

#[test]
fn scope_set_rejects_equal_specificity_provider_claims() {
    let first = SemanticWorkspaceScope::from_packet(&packet()).expect("first scope");
    let mut second_packet = packet();
    second_packet["workspaceId"] = json!("python:other");
    second_packet["providerId"] = json!("other-python-harness");
    let second = SemanticWorkspaceScope::from_packet(&second_packet).expect("second scope");
    let scopes = SemanticWorkspaceScopeSet::new(vec![first, second]).expect("scope set");

    let rejection = scopes
        .admit_candidate_from(Path::new("/work"), Path::new("root/src/app.py"))
        .expect_err("ambiguous providers");
    assert_eq!(rejection.reason_kind, "candidate-provider-ambiguous");
}

#[test]
fn rejects_unowned_or_relative_admitted_roots() {
    let mut unknown = packet();
    unknown["admittedRoots"] = json!(["/work/root", "/work/shared", "/work/other"]);
    assert!(SemanticWorkspaceScope::from_packet(&unknown).is_err());

    let mut relative = packet();
    relative["admittedRoots"][0] = json!("root");
    assert!(SemanticWorkspaceScope::from_packet(&relative).is_err());
}

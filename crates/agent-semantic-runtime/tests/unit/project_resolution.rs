use super::{
    AdmittedProjectResolution, LANGUAGE_PACKAGE_GRAPH_SCHEMA_ID, PROJECT_RESOLUTION_SCHEMA_ID,
    ProjectResolutionReceipt, workspace_source_scope_generation_digest,
};

fn scope(package_name: &str) -> ProjectResolutionReceipt {
    serde_json::from_value(serde_json::json!({
        "schemaId": PROJECT_RESOLUTION_SCHEMA_ID,
        "schemaVersion": "1",
        "state": "resolved",
        "completeness": "exact",
        "languageId": "rust",
        "providerId": "rs-harness",
        "parserId": "rust.cargo-toml",
        "candidateGenerationDigest": "blake3-256:candidates",
        "projectEntry": "Cargo.toml",
        "packageGraph": {
            "schemaId": LANGUAGE_PACKAGE_GRAPH_SCHEMA_ID,
            "schemaVersion": "1",
            "languageId": "rust",
            "providerId": "rs-harness",
            "projectEntry": "Cargo.toml",
            "parserId": "rust.cargo-toml",
            "manifests": [{"path": "Cargo.toml", "kind": "cargo-manifest", "digest": "blake3-256:manifest"}],
            "lockfiles": [],
            "packages": [{
                "packageId": "root",
                "name": package_name,
                "manifestPath": "Cargo.toml",
                "root": ".",
                "workspaceMember": true,
                "targets": [{
                    "targetId": "root:lib",
                    "kind": "lib",
                    "name": package_name,
                    "explicit": true,
                    "sourceRoots": ["src"],
                    "entrypoints": ["src/lib.rs"],
                    "generatedRoots": []
                }]
            }],
            "internalDependencyEdges": [],
            "externalDependencies": [],
            "unresolved": []
        },
        "sourceScopes": [{
            "scopeId": "root:lib",
            "packageId": "root",
            "targetId": "root:lib",
            "roots": ["src"],
            "explicitPaths": ["src/lib.rs"],
            "extensions": [".rs"],
            "includeAuthority": "package-manager",
            "exclusions": [],
            "classifications": ["production"]
        }],
        "conflicts": [],
        "metrics": {
            "parsedManifestCount": 1,
            "parsedLockfileCount": 0,
            "affectedPackageCount": 1,
            "fullWorkspaceReads": 0,
            "fullManifestReparses": 0,
            "dbOpens": 0,
            "elapsedMicros": 10
        }
    }))
    .expect("typed ProjectResolution fixture")
}

#[test]
fn project_resolution_is_provider_semantics_without_workspace_identity() {
    let scope = scope("fixture");
    scope
        .validate("rust", "rs-harness", "blake3-256:candidates")
        .expect("valid provider ProjectResolution");
    let encoded = serde_json::to_value(&scope).expect("encode ProjectResolution");
    for forbidden in [
        "repositoryIdentity",
        "worktreeIdentity",
        "workspaceIdentity",
        "repositoryCandidates",
    ] {
        assert!(
            encoded.get(forbidden).is_none(),
            "forbidden field {forbidden}"
        );
    }
}

#[test]
fn project_resolution_metrics_are_json_safe_at_the_v1_integer_boundary() {
    let mut receipt = scope("json-boundary");
    receipt.metrics.elapsed_micros = u64::MAX;
    let encoded = serde_json::to_value(&receipt)
        .expect("v1 ProjectResolution metrics must remain representable on Runtime IPC");
    assert_eq!(
        encoded["metrics"]["elapsedMicros"],
        serde_json::json!(u64::MAX)
    );
}

#[test]
fn package_graph_and_candidate_base_are_generation_identity() {
    let first = AdmittedProjectResolution::new("packages/first", scope("first"))
        .expect("first admitted scope");
    let second = AdmittedProjectResolution::new("packages/first", scope("second"))
        .expect("second admitted scope");
    let rebased = AdmittedProjectResolution::new("packages/second", scope("first"))
        .expect("rebased admitted scope");
    assert_ne!(
        workspace_source_scope_generation_digest(std::slice::from_ref(&first)).unwrap(),
        workspace_source_scope_generation_digest(std::slice::from_ref(&second)).unwrap()
    );
    assert_ne!(
        workspace_source_scope_generation_digest(std::slice::from_ref(&first)).unwrap(),
        workspace_source_scope_generation_digest(std::slice::from_ref(&rebased)).unwrap()
    );
}

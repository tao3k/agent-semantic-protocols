use std::path::Path;

use super::{
    ProviderProjectResolutionFile, ProviderProjectResolutionPacket,
    provider_project_resolution_files_from_packet,
};

#[test]
fn project_resolution_request_carries_typed_candidate_generation_identity() {
    let provider = rust_project_resolution_provider();
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root");
    let repository_candidates =
        agent_semantic_runtime::git::discover_repository_candidate_snapshot(project_root)
            .expect("discover repository candidates")
            .expect("Git candidate snapshot");
    let (_, request, _) = super::provider_project_resolution_invocation_with_candidates(
        agent_semantic_provider_transport::ProviderProcessSupervisor::default(),
        &provider,
        project_root,
        &super::ProviderProjectResolutionCollectionScope::CompleteGeneration,
        repository_candidates,
    )
    .expect("build provider project-resolution request");
    let stdin = request.stdin.expect("project-resolution stdin");
    let value: serde_json::Value =
        serde_json::from_slice(&stdin).expect("parse project-resolution request");

    assert_eq!(
        value["candidateGeneration"]["algorithm"],
        "blake3-worktree-state-v1"
    );
}

#[test]
fn provider_invocation_rebases_repository_snapshot_before_language_harness() {
    let provider = rust_project_resolution_provider();
    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repository root");
    let project_root = repository_root.join("build-support/asp-rust-project-harness-policy");
    let repository_candidates =
        agent_semantic_runtime::git::discover_repository_candidate_snapshot(repository_root)
            .expect("discover repository candidates")
            .expect("Git candidate snapshot");

    let (_, request, _) = super::provider_project_resolution_invocation_with_candidates(
        agent_semantic_provider_transport::ProviderProcessSupervisor::default(),
        &provider,
        &project_root,
        &super::ProviderProjectResolutionCollectionScope::CompleteGeneration,
        repository_candidates,
    )
    .expect("build scoped provider request");
    let value: serde_json::Value =
        serde_json::from_slice(&request.stdin.expect("project-resolution stdin"))
            .expect("parse project-resolution request");
    assert_eq!(value["candidateBase"], ".");
    let paths = value["candidatePaths"]
        .as_array()
        .expect("candidate array")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect::<Vec<_>>();
    assert!(paths.contains(&"Cargo.toml"));
    assert!(
        paths
            .iter()
            .all(|path| !path.starts_with("build-support/asp-rust-project-harness-policy/"))
    );
}

#[test]
fn provider_project_resolution_packet_is_the_only_file_scope_input() {
    let scope = provider_project_resolution_files_from_packet(
        Path::new("."),
        Path::new("."),
        ProviderProjectResolutionPacket {
            language_id: "rust".into(),
            provider_id: "rust-lang-project-harness".into(),
            resolution: project_resolution_receipt(),
            files: Vec::new(),
        },
    );
    assert!(scope.is_empty());
}

#[test]
fn provider_project_resolution_packet_does_not_materialize_missing_files() {
    let files = provider_project_resolution_files_from_packet(
        Path::new("."),
        Path::new("."),
        ProviderProjectResolutionPacket {
            language_id: "rust".into(),
            provider_id: "rust-lang-project-harness".into(),
            resolution: project_resolution_receipt(),
            files: vec![ProviderProjectResolutionFile {
                path: "this-file-does-not-exist.rs".into(),
                language_id: "rust".into(),
                provider_id: "rust-lang-project-harness".into(),
            }],
        },
    );
    assert!(files.is_empty());
}

fn rust_project_resolution_provider() -> agent_semantic_client_core::ResolvedProvider {
    let manifest = agent_semantic_hook::builtin_provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id().as_str() == "rust")
        .expect("Rust provider manifest");
    agent_semantic_client_core::ResolvedProvider {
        scope_authority: agent_semantic_client_core::ProviderScopeAuthority::ProjectResolution,
        manifest_id: "rs-harness-test-manifest".to_string(),
        manifest_digest: "sha256:rs-harness-test-manifest".to_string(),
        namespace: "rust".to_string(),
        language_id: "rust".into(),
        provider_id: "rs-harness".into(),
        binary: "rs-harness".to_string(),
        execution: agent_semantic_hook::ProviderExecution::ExternalProcess,
        provider_command_prefix: vec!["rs-harness".to_string()],
        execution_command_digest: "test-execution-command-digest".to_string(),
        runtime_command_argv: None,
        runtime_profile_status: None,
        package_roots: vec![".".to_string()],
        config_files: Vec::new(),
        source_extensions: vec![".rs".to_string()],
        search_capabilities: manifest.search_capabilities().clone(),
        language_projection: manifest.language_projection().cloned(),
        query_pack_descriptor: manifest.query_pack_descriptor().clone(),
        semantic_facts_descriptor: manifest.semantic_facts_descriptor().cloned(),
    }
}

#[tokio::test]
async fn async_scope_file_admission_is_deterministic_and_ignores_missing_files() {
    static FIXTURE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "asp-provider-scope-{}-{}",
        std::process::id(),
        FIXTURE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    tokio::fs::create_dir_all(root.join("src"))
        .await
        .expect("create provider scope fixture");
    tokio::fs::write(root.join("src/z.rs"), b"fn z() {}\n")
        .await
        .expect("write z owner");
    tokio::fs::write(root.join("src/a.rs"), b"fn a() {}\n")
        .await
        .expect("write a owner");
    let packet = ProviderProjectResolutionPacket {
        language_id: "rust".into(),
        provider_id: "rs-harness".into(),
        resolution: project_resolution_receipt(),
        files: ["src/z.rs", "missing.rs", "src/a.rs"]
            .into_iter()
            .map(|path| ProviderProjectResolutionFile {
                path: path.into(),
                language_id: "rust".into(),
                provider_id: "rs-harness".into(),
            })
            .collect(),
    };

    let admitted = super::provider_project_resolution_files_from_packet_async(&root, &root, packet)
        .await
        .expect("admit async scope files");
    assert_eq!(
        admitted
            .iter()
            .map(|file| {
                file.path
                    .strip_prefix(&root)
                    .expect("fixture-relative owner")
                    .to_string_lossy()
                    .to_string()
            })
            .collect::<Vec<_>>(),
        vec!["src/a.rs".to_owned(), "src/z.rs".to_owned()]
    );
    tokio::fs::remove_dir_all(root)
        .await
        .expect("remove provider scope fixture");
}

fn candidate_fixture(
    paths: &[&str],
    policy_exclusions: Vec<super::ProviderProjectResolutionPolicyExclusion>,
) -> super::ProviderProjectResolutionCandidates {
    super::ProviderProjectResolutionCandidates {
        generation_digest: "candidate-v1".to_owned(),
        generation: serde_json::json!({
            "algorithm": "blake3-worktree-state-v1",
            "digest": "candidate-v1",
            "authorities": ["git-index"]
        }),
        paths: paths.iter().map(|path| (*path).to_owned()).collect(),
        policy_exclusions,
    }
}

#[test]
fn not_applicable_provider_resolution_maps_to_unsupported() {
    let stdout = serde_json::to_vec(&serde_json::json!({
        "schemaId": "agent.semantic-protocols.provider-project-resolution-response",
        "schemaVersion": "1",
        "languageId": "python",
        "providerId": "py-harness",
        "state": "not-applicable"
    }))
    .expect("encode not-applicable provider response");
    let resolution = super::project_resolution_from_stdout(
        &stdout,
        &super::LanguageId::from("python"),
        &super::ProviderId::from("py-harness"),
        &candidate_fixture(&[], Vec::new()),
    )
    .expect("not-applicable provider response");
    assert!(matches!(
        resolution,
        super::ProviderProjectResolution::Unsupported
    ));
}

fn project_resolution_scope(source_scopes: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "schemaId": "agent.semantic-protocols.project-resolution",
        "schemaVersion": "1",
        "state": "resolved",
        "completeness": "exact",
        "languageId": "rust",
        "providerId": "rs-harness",
        "parserId": "rust.cargo-toml",
        "candidateGenerationDigest": "candidate-v1",
        "projectEntry": "Cargo.toml",
        "packageGraph": {
            "schemaId": "agent.semantic-protocols.language-package-graph",
            "schemaVersion": "1",
            "languageId": "rust",
            "providerId": "rs-harness",
            "projectEntry": "Cargo.toml",
            "parserId": "rust.cargo-toml",
            "manifests": [{
                "path": "Cargo.toml",
                "kind": "cargo-manifest",
                "digest": "blake3-256:manifest"
            }],
            "lockfiles": [],
            "packages": [{
                "packageId": "root",
                "name": "fixture",
                "manifestPath": "Cargo.toml",
                "root": ".",
                "workspaceMember": true,
                "targets": [{
                    "targetId": "root:lib",
                    "kind": "lib",
                    "name": "fixture",
                    "explicit": true,
                    "sourceRoots": ["src", "generated"],
                    "entrypoints": ["src/lib.rs", "generated/lib.rs"],
                    "generatedRoots": []
                }]
            }],
            "internalDependencyEdges": [],
            "externalDependencies": [],
            "unresolved": []
        },
        "sourceScopes": source_scopes,
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
    })
}

fn project_resolution_receipt() -> agent_semantic_runtime::ProjectResolutionReceipt {
    serde_json::from_value(project_resolution_scope(serde_json::json!([{
        "scopeId": "root:lib",
        "packageId": "root",
        "targetId": "root:lib",
        "roots": ["src"],
        "explicitPaths": [],
        "extensions": [".rs"],
        "includeAuthority": "package-manager",
        "exclusions": [],
        "classifications": ["production"]
    }])))
    .expect("typed ProjectResolution fixture")
}

fn project_resolution_response(source_scopes: serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "schemaId": "agent.semantic-protocols.provider-project-resolution-response",
        "schemaVersion": "1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "state": "resolved",
        "scope": project_resolution_scope(source_scopes)
    }))
    .expect("encode ProjectResolution response fixture")
}

#[test]
fn non_schema_resolution_field_is_rejected() {
    let stdout = br#"{
      "schemaId":"agent.semantic-protocols.provider-project-resolution-response",
      "schemaVersion":"1",
      "languageId":"rust",
      "providerId":"rs-harness",
      "state":"resolved",
      "resolution":{}
    }"#;
    let error = super::project_resolution_from_stdout(
        stdout,
        &super::LanguageId::from("rust"),
        &super::ProviderId::from("rs-harness"),
        &candidate_fixture(&[], Vec::new()),
    )
    .expect_err("non-schema resolution field must remain unsupported");

    assert!(error.contains("omitted scope"), "{error}");
}

#[test]
fn project_resolution_resolves_only_git_candidates() {
    let stdout = project_resolution_response(serde_json::json!([{
        "scopeId":"root:lib",
        "packageId":"root",
        "targetId":"root:lib",
        "roots":["src"],
        "explicitPaths":[],
        "extensions":[".rs"],
        "includeAuthority":"package-manager",
        "exclusions":[{"prefix":"src/generated.rs","authority":"package-manager"}],
        "classifications":["production"]
    }]));
    let candidates = candidate_fixture(
        &[
            "src/lint_babel.rs",
            "src/lib.rs",
            "src/generated.rs",
            "README.md",
        ],
        Vec::new(),
    );
    let scope = super::project_resolution_from_stdout(
        &stdout,
        &super::LanguageId::from("rust"),
        &super::ProviderId::from("rs-harness"),
        &candidates,
    )
    .expect("project-resolution response should define the source scope");
    let super::ProviderProjectResolution::Supported(packet) = scope else {
        panic!("project-resolution response must not become unsupported");
    };
    assert_eq!(packet.resolution.project_entry, "Cargo.toml");
    assert_eq!(
        packet
            .files
            .into_iter()
            .map(|file| file.path)
            .collect::<Vec<_>>(),
        vec!["src/lib.rs", "src/lint_babel.rs"]
    );
}

#[test]
fn project_resolution_applies_typed_policy_exclusions_once() {
    let stdout = project_resolution_response(serde_json::json!([{
        "scopeId":"root:lib",
        "packageId":"root",
        "targetId":"root:lib",
        "roots":["src"],
        "explicitPaths":[],
        "extensions":[".rs"],
        "includeAuthority":"package-manager",
        "exclusions":[],
        "classifications":["production"]
    }]));
    let candidates = candidate_fixture(
        &["src/lib.rs", "src/generated.rs"],
        vec![super::ProviderProjectResolutionPolicyExclusion {
            path: "src/generated.rs".to_owned(),
            authority: "user-policy".to_owned(),
            reason_kind: "configured-exclusion".to_owned(),
        }],
    );
    let scope = super::project_resolution_from_stdout(
        &stdout,
        &super::LanguageId::from("rust"),
        &super::ProviderId::from("rs-harness"),
        &candidates,
    )
    .expect("ASP policy should filter an implicit package-manager scope");
    let super::ProviderProjectResolution::Supported(packet) = scope else {
        panic!("project-resolution response must not become unsupported");
    };
    assert_eq!(
        packet
            .files
            .into_iter()
            .map(|file| file.path)
            .collect::<Vec<_>>(),
        vec!["src/lib.rs"]
    );
}

#[test]
fn explicit_package_target_and_policy_exclusion_fail_closed() {
    let stdout = project_resolution_response(serde_json::json!([{
        "scopeId":"root:lib",
        "packageId":"root",
        "targetId":"root:lib",
        "roots":["generated"],
        "explicitPaths":["generated/lib.rs"],
        "extensions":[".rs"],
        "includeAuthority":"manifest-explicit",
        "exclusions":[],
        "classifications":["generated"]
    }]));
    let candidates = candidate_fixture(
        &["generated/lib.rs"],
        vec![super::ProviderProjectResolutionPolicyExclusion {
            path: "generated/lib.rs".to_owned(),
            authority: "user-policy".to_owned(),
            reason_kind: "configured-exclusion".to_owned(),
        }],
    );
    let error = super::project_resolution_from_stdout(
        &stdout,
        &super::LanguageId::from("rust"),
        &super::ProviderId::from("rs-harness"),
        &candidates,
    )
    .expect_err("explicit package target exclusion must fail closed");

    assert!(error.contains("project-resolution-conflict"), "{error}");
    assert!(error.contains("path=generated/lib.rs"), "{error}");
    assert!(error.contains("excludeAuthority=user-policy"), "{error}");
}

#[test]
fn provider_cannot_inject_asp_owned_policy_exclusions() {
    let stdout = project_resolution_response(serde_json::json!([{
        "scopeId":"root:lib",
        "packageId":"root",
        "targetId":"root:lib",
        "roots":["src"],
        "explicitPaths":[],
        "extensions":[".rs"],
        "includeAuthority":"package-manager",
        "exclusions":[{"prefix":"src/lib.rs","authority":"user-policy"}],
        "classifications":["production"]
    }]));
    let candidates = candidate_fixture(&["src/lib.rs"], Vec::new());
    let error = super::project_resolution_from_stdout(
        &stdout,
        &super::LanguageId::from("rust"),
        &super::ProviderId::from("rs-harness"),
        &candidates,
    )
    .expect_err("provider must not inject ASP-owned user policy");

    assert!(
        error.contains("provider-project-resolution-invalid-exclusion-authority"),
        "{error}"
    );
}

use super::{SourceIndexCollectionScopeV1, collect_workspace_search_source_index_files};
use agent_semantic_client_core::{ProviderExecution, ProviderRegistrySnapshot, ResolvedProvider};
use std::time::{SystemTime, UNIX_EPOCH};

fn provider(language_id: &str, provider_id: &str) -> ResolvedProvider {
    let manifest = agent_semantic_hook::builtin_provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id().as_str() == "rust")
        .expect("rust provider manifest");
    ResolvedProvider {
        manifest_id: format!("{provider_id}-test-manifest"),
        manifest_digest: format!("sha256:{provider_id}-test-manifest"),
        namespace: language_id.to_string(),
        language_id: language_id.into(),
        provider_id: provider_id.into(),
        binary: provider_id.to_string(),
        execution: ProviderExecution::ExternalProcess,
        provider_command_prefix: Vec::new(),
        execution_command_digest: "test-execution-command-digest".to_string(),
        runtime_command_argv: None,
        runtime_profile_status: None,
        package_roots: Vec::new(),
        source_roots: Vec::new(),
        config_files: Vec::new(),
        source_extensions: Vec::new(),
        ignored_path_prefixes: Vec::new(),
        search_capabilities: manifest.search_capabilities().clone(),
        query_pack_descriptor: manifest.query_pack_descriptor().clone(),
        semantic_facts_descriptor: manifest.semantic_facts_descriptor().cloned(),
    }
}

fn fixture() -> (std::path::PathBuf, ProviderRegistrySnapshot) {
    let root = std::env::temp_dir().join(format!(
        "source-index-collection-scope-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos()
    ));
    std::fs::create_dir_all(root.join("src")).expect("create Rust source root");
    std::fs::write(root.join("src/lib.rs"), "pub fn fixture() {}\n").expect("write Rust source");

    let mut rust = provider("rust", "rs-harness");
    rust.source_roots = vec!["src".to_string()];
    rust.source_extensions = vec!["rs".to_string()];
    let mut gerbil = provider("gerbil-scheme", "gerbil-scheme-harness");
    gerbil.source_roots = vec!["missing-gerbil-source".to_string()];
    gerbil.source_extensions = vec!["ss".to_string()];
    let snapshot = ProviderRegistrySnapshot {
        activation_path: root.join("registry.json"),
        providers: vec![rust, gerbil],
    };
    (root, snapshot)
}

#[test]
fn target_language_provider_does_not_require_unrelated_provider_coverage() {
    let (root, snapshot) = fixture();
    let files = collect_workspace_search_source_index_files(
        &root,
        &snapshot,
        &SourceIndexCollectionScopeV1::TargetProvider {
            language_id: "rust".into(),
            provider_id: "rs-harness".into(),
        },
    )
    .expect("collect target Rust provider");

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].language_id.as_str(), "rust");
    assert_eq!(files[0].provider_id.as_str(), "rs-harness");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn complete_generation_requires_every_provider_coverage() {
    let (root, snapshot) = fixture();
    let error = collect_workspace_search_source_index_files(
        &root,
        &snapshot,
        &SourceIndexCollectionScopeV1::CompleteGeneration,
    )
    .expect_err("complete generation must fail on missing Gerbil coverage");

    assert!(error.contains("missingProviderIds=gerbil-scheme-harness"));
    let _ = std::fs::remove_dir_all(root);
}

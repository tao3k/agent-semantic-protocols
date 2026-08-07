use super::{
    ProviderScopeCollectionRoute, SourceIndexCollectionScope, provider_scope_collection_route,
    retain_explicit_owner_files,
};
use agent_semantic_client_core::ProviderScopeAuthority;

#[test]
fn package_providers_use_project_resolution() {
    assert_eq!(
        provider_scope_collection_route(ProviderScopeAuthority::ProjectResolution),
        ProviderScopeCollectionRoute::ProjectResolution
    );
}

#[test]
fn document_providers_use_git_candidates_without_project_resolution() {
    assert_eq!(
        provider_scope_collection_route(ProviderScopeAuthority::DocumentResolution),
        ProviderScopeCollectionRoute::GitDocumentCandidates
    );
}

fn scope_file(path: std::path::PathBuf) -> agent_semantic_client_db::ClientDbSourceIndexScopeFile {
    agent_semantic_client_db::ClientDbSourceIndexScopeFile {
        path,
        language_id: agent_semantic_client_core::LanguageId::new("rust").expect("test language id"),
        provider_id: agent_semantic_client_core::ProviderId::new("rs-harness")
            .expect("test provider id"),
        projection_coverage:
            agent_semantic_client_db::ClientDbSourceIndexProjectionCoverage::NotDeclared,
        selector_receipts: Vec::new(),
        relations: Vec::new(),
    }
}

#[test]
fn explicit_owner_collection_filters_only_after_provider_scope_resolution() {
    let root = std::path::Path::new("/workspace/project");
    let mut files = vec![
        scope_file(root.join("Cargo.toml")),
        scope_file(root.join("src/changed.rs")),
        scope_file(root.join("src/unchanged.rs")),
    ];

    retain_explicit_owner_files(
        root,
        &SourceIndexCollectionScope::ExplicitOwners {
            owner_paths: vec!["src/changed.rs".to_owned()],
        },
        &mut files,
    )
    .expect("filter provider result by explicit owner identity");

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, root.join("src/changed.rs"));
}

#[test]
fn explicit_owner_collection_rejects_non_normalized_or_absolute_paths() {
    let root = std::path::Path::new("/workspace/project");
    for owner_path in ["../outside.rs", "./src/lib.rs", "/outside.rs"] {
        let error = retain_explicit_owner_files(
            root,
            &SourceIndexCollectionScope::ExplicitOwners {
                owner_paths: vec![owner_path.to_owned()],
            },
            &mut Vec::new(),
        )
        .expect_err("invalid explicit owner path must fail closed");
        assert!(error.contains("normalized and workspace-relative"));
    }
}

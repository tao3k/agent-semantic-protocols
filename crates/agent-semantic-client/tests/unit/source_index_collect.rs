use super::{
    ProviderSourceInventoryAdapter, SourceIndexCollectionScope, retain_explicit_owner_files,
    select_provider_source_inventory_adapter,
};

fn inventory_capabilities(
    project_entry_markers: Option<Vec<&str>>,
    document_git_candidates: Option<bool>,
) -> agent_semantic_client_core::ProviderSourceInventoryCapabilities {
    agent_semantic_client_core::ProviderSourceInventoryCapabilities {
        project_resolution: project_entry_markers.map(|entry_markers| {
            agent_semantic_client_core::ProviderProjectInventoryCapability {
                entry_markers: entry_markers.into_iter().map(str::to_owned).collect(),
            }
        }),
        document_resolution: document_git_candidates.map(|supports_git_candidates| {
            agent_semantic_client_core::ProviderDocumentInventoryCapability {
                extensions: vec![".ss".to_owned()],
                supports_git_candidates,
            }
        }),
    }
}

#[test]
fn exact_project_entry_selects_project_adapter_when_both_capabilities_are_declared() {
    let capabilities = inventory_capabilities(Some(vec!["gerbil.pkg"]), Some(true));
    assert_eq!(
        select_provider_source_inventory_adapter(
            &capabilities,
            "gerbil-scheme",
            "asp-gerbil-scheme",
            |path| path == "gerbil.pkg",
        ),
        Ok(ProviderSourceInventoryAdapter::ProjectResolution)
    );
}

#[test]
fn absent_project_entry_selects_declared_document_adapter_before_invocation() {
    let capabilities = inventory_capabilities(Some(vec!["gerbil.pkg"]), Some(true));
    assert_eq!(
        select_provider_source_inventory_adapter(
            &capabilities,
            "gerbil-scheme",
            "asp-gerbil-scheme",
            |_| false,
        ),
        Ok(ProviderSourceInventoryAdapter::GitDocumentCandidates)
    );
}

#[test]
fn absent_project_entry_without_document_capability_fails_closed() {
    let capabilities = inventory_capabilities(Some(vec!["Cargo.toml"]), None);
    let error =
        select_provider_source_inventory_adapter(&capabilities, "rust", "asp-rust", |_| false)
            .expect_err("missing declared adapter must fail closed");
    assert!(error.contains("reasonKind=provider-source-inventory-capability-unavailable"));
}

#[test]
fn document_adapter_without_git_candidate_support_fails_closed() {
    let capabilities = inventory_capabilities(None, Some(false));
    let error = select_provider_source_inventory_adapter(
        &capabilities,
        "gerbil-scheme",
        "asp-gerbil-scheme",
        |_| false,
    )
    .expect_err("document adapter precondition must fail closed");
    assert!(error.contains("reasonKind=provider-document-resolution-git-candidates-unsupported"));
}

fn scope_file(path: std::path::PathBuf) -> agent_semantic_client_db::ClientDbSourceIndexScopeFile {
    agent_semantic_client_db::ClientDbSourceIndexScopeFile {
        path,
        language_id: agent_semantic_client_core::LanguageId::new("rust"),
        provider_id: agent_semantic_client_core::ProviderId::new("rs-harness"),
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

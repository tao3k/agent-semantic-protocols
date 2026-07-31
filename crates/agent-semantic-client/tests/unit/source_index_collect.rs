use super::{ProviderScopeCollectionRoute, provider_scope_collection_route};
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

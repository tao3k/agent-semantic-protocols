use agent_semantic_client_core::ProviderScopeAuthority;
use agent_semantic_client_local_cli::provider_scope_authority_permits_project_resolution;

#[test]
fn project_resolution_authority_admits_package_scope() {
    assert!(provider_scope_authority_permits_project_resolution(
        &ProviderScopeAuthority::ProjectResolution,
    ));
}

#[test]
fn document_resolution_authority_cannot_invoke_package_scope() {
    assert!(!provider_scope_authority_permits_project_resolution(
        &ProviderScopeAuthority::DocumentResolution,
    ));
}

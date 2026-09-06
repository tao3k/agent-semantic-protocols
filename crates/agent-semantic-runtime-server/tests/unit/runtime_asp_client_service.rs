use super::workspace_search_providers_from_provider_register;

#[test]
fn workspace_search_provider_snapshot_uses_admitted_register_facts() {
    let register =
        agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister::from_seed(
            agent_semantic_provider_protocol::builtin_provider_registrations()
                .expect("builtin provider registrations"),
        )
        .expect("validated provider register");
    let providers = workspace_search_providers_from_provider_register(&register)
        .expect("immutable workspace Search provider snapshot");
    assert!(!providers.is_empty());
    assert!(providers.iter().all(|provider| {
        provider.search_supported
            && !provider.source_extensions.is_empty()
            && provider
                .source_extensions
                .iter()
                .all(|extension| !extension.starts_with('.'))
    }));
    assert!(
        providers
            .windows(2)
            .all(|pair| pair[0].language_id < pair[1].language_id)
    );
}

#[tokio::test]
async fn workspace_search_provider_snapshot_observes_register_refresh() {
    let mut registrations = agent_semantic_provider_protocol::builtin_provider_registrations()
        .expect("builtin provider registrations");
    let rust_index = registrations
        .iter()
        .position(|provider| provider.language_id == "rust")
        .expect("Rust provider");
    let refreshed = registrations[rust_index].clone();
    registrations[rust_index]
        .registration
        .as_object_mut()
        .expect("provider registration object")
        .remove("searchPlaybookContract");
    let register =
        agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister::from_seed(
            registrations,
        )
        .expect("validated provider register");
    let before = workspace_search_providers_from_provider_register(&register)
        .expect("provider snapshot before refresh");
    assert!(
        before
            .iter()
            .find(|provider| provider.language_id == "rust")
            .expect("Rust provider before refresh")
            .search_playbook_contract
            .is_none()
    );

    let generation = register.snapshot().generation;
    let response = register
        .apply(agent_semantic_provider_protocol::ProviderRegisterRequest {
            schema_id: agent_semantic_provider_protocol::PROVIDER_REGISTER_REQUEST_SCHEMA_ID
                .to_owned(),
            schema_version: agent_semantic_provider_protocol::PROVIDER_REGISTER_SCHEMA_VERSION
                .to_owned(),
            expected_generation: Some(generation),
            request: agent_semantic_provider_protocol::ProviderRegisterOperation::Register {
                provider: refreshed,
            },
        })
        .await
        .expect("refresh provider register");
    response.validate().expect("valid register refresh");

    let after = workspace_search_providers_from_provider_register(&register)
        .expect("provider snapshot after refresh");
    assert!(
        after
            .iter()
            .find(|provider| provider.language_id == "rust")
            .expect("Rust provider after refresh")
            .search_playbook_contract
            .is_some()
    );
}

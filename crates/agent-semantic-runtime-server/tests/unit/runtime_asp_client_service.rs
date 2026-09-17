// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::workspace_search_providers_from_provider_register;

#[test]
fn initialized_workspace_retains_gix_runtime_key_and_parser_host_binding() {
    let directory = tempfile::tempdir().expect("temporary project root");
    let project_root = directory.path().to_path_buf();
    let manifest_binding = agent_semantic_content_identity::ProjectWorkspaceBinding::new(
        "git+https://github.com/tao3k/agent-semantic-protocols.git#workspace/main",
        ".",
        "cross-machine",
        vec!["git+ssh://git@github.com/tao3k/agent-semantic-protocols.git".to_owned()],
    )
    .expect("valid parser-owned Project Workspace binding");
    let expected = manifest_binding.clone();
    let resolver: super::HostWorkspaceInitializationBindingResolver =
        std::sync::Arc::new(move |root: &std::path::Path| {
            assert_eq!(root, project_root);
            agent_semantic_content_identity::HostWorkspaceInitializationBinding::new(
                manifest_binding.clone(),
                "worktree:test",
            )
            .map_err(|error| error.to_string())
        });

    let initialized = super::InitializedWorkspace::from_project_root(
        directory.path().to_path_buf(),
        &resolver,
        "route-project",
        "route-workspace",
    )
    .expect("manifest-admitted initialized workspace");

    assert_eq!(initialized.host_workspace.project_workspace(), &expected);
    assert_eq!(
        initialized.host_workspace.worktree_instance_id(),
        "worktree:test"
    );
    assert_eq!(
        initialized
            .runtime_workspace_key
            .routing_project_id()
            .as_str(),
        "route-project"
    );
    assert_eq!(
        initialized
            .runtime_workspace_key
            .routing_workspace_id()
            .as_str(),
        "route-workspace"
    );
}

#[test]
fn workspace_search_provider_snapshot_uses_admitted_register_facts() {
    let register =
        agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister::from_seed(
            agent_semantic_provider_protocol::builtin_provider_registrations()
                .expect("builtin provider registrations"),
        )
        .expect("validated provider register");
    let schema_bundles = crate::RuntimeSchemaBundleCatalog::load_embedded()
        .expect("embedded Runtime schema bundle catalog");
    let providers = workspace_search_providers_from_provider_register(&register, &schema_bundles)
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
    assert!(
        providers
            .iter()
            .any(|provider| provider.language_id == "rust")
    );
    let org = providers
        .iter()
        .find(|provider| provider.language_id == "org")
        .expect("embedded Org producer");
    assert_eq!(org.provider_id, "asp-org");
    assert_eq!(org.source_extensions, ["org", "org_archive"]);
    assert_eq!(
        org.producer_axes,
        [agent_semantic_search::WorkspaceSearchProducerAxis::Document]
    );
    assert_eq!(
        schema_bundles
            .search_producer_axes("org")
            .expect("registered Org schema profile"),
        [agent_semantic_schema_manager::SearchProducerAxis::Document]
    );
}

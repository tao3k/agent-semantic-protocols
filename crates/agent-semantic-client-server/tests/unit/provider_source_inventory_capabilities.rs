// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_client_core::ProviderDocumentInventoryCapability;
use agent_semantic_client_core::ProviderProjectInventoryCapability;
use agent_semantic_client_core::ProviderSourceInventoryCapabilities;
use agent_semantic_client_server::provider_capabilities_permit_project_resolution;

#[test]
fn declared_project_resolution_capability_admits_package_scope() {
    assert!(provider_capabilities_permit_project_resolution(
        &ProviderSourceInventoryCapabilities {
            project_resolution: Some(ProviderProjectInventoryCapability {
                entry_markers: vec!["Cargo.toml".to_owned()],
            }),
            document_resolution: None,
        },
    ));
}

#[test]
fn document_only_capability_cannot_invoke_package_scope() {
    assert!(!provider_capabilities_permit_project_resolution(
        &ProviderSourceInventoryCapabilities {
            project_resolution: None,
            document_resolution: Some(ProviderDocumentInventoryCapability {
                extensions: vec![".md".to_owned()],
                supports_git_candidates: true,
            }),
        },
    ));
}

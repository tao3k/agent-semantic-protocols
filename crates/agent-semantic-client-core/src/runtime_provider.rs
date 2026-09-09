// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Runtime-owned provider projection consumed by semantic clients.

use std::collections::BTreeSet;
use std::path::Path;

use crate::receipt::NativeProvenance;
use crate::types::LanguageId;
use crate::types::ProviderId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProviderOperation {
    pub operation: String,
    pub request_schema_id: String,
    pub response_schema_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProvider {
    pub registration_digest: String,
    pub namespace: String,
    pub language_id: LanguageId,
    pub provider_id: ProviderId,
    /// Runtime-owned materialized provider entrypoint.
    pub binary: String,
    pub package_roots: Vec<String>,
    pub config_files: Vec<String>,
    pub source_extensions: Vec<String>,
    pub source_inventory_capabilities: ProviderSourceInventoryCapabilities,
    pub search_capabilities: agent_semantic_provider_protocol::ProviderSearchCapabilities,
    pub query_pack_descriptor: agent_semantic_provider_protocol::ProviderQueryPackDescriptor,
    pub semantic_facts_descriptor:
        Option<agent_semantic_provider_protocol::ProviderSemanticFactsDescriptor>,
    pub runtime_operations: Vec<RuntimeProviderOperation>,
}

impl RuntimeProvider {
    #[must_use]
    pub fn provenance(&self) -> NativeProvenance {
        NativeProvenance {
            provider_binary: "asp".to_string(),
            language_id: self.language_id.clone(),
            provider_id: self.provider_id.clone(),
        }
    }

    #[must_use]
    pub fn runtime_operation(&self, operation: &str) -> Option<&RuntimeProviderOperation> {
        self.runtime_operations
            .iter()
            .find(|item| item.operation == operation)
    }
}

/// Immutable view assembled by ASP Server from its live register and installed artifacts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProviderProjection {
    pub authority_ref: String,
    pub providers: Vec<RuntimeProvider>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProviderProjectionEvidence {
    pub fingerprint: String,
    pub scope_dirs: BTreeSet<String>,
}

impl RuntimeProviderProjection {
    #[must_use]
    pub fn provider_for_language(&self, language_id: &LanguageId) -> Option<&RuntimeProvider> {
        self.providers
            .iter()
            .find(|provider| &provider.language_id == language_id)
    }

    #[must_use]
    pub fn native_provenance(&self) -> Vec<NativeProvenance> {
        self.providers
            .iter()
            .map(RuntimeProvider::provenance)
            .collect()
    }

    #[must_use]
    pub fn evidence(&self, project_root: &Path) -> RuntimeProviderProjectionEvidence {
        RuntimeProviderProjectionEvidence {
            fingerprint: runtime_provider_projection_fingerprint(self),
            scope_dirs: runtime_provider_projection_scope_dirs(project_root, self),
        }
    }
}

fn runtime_provider_projection_fingerprint(projection: &RuntimeProviderProjection) -> String {
    let mut rows = vec![format!("authority={}", projection.authority_ref)];
    for provider in &projection.providers {
        rows.push(runtime_provider_fingerprint(provider));
    }
    rows.join("\n")
}

fn runtime_provider_projection_scope_dirs(
    project_root: &Path,
    projection: &RuntimeProviderProjection,
) -> BTreeSet<String> {
    let mut dirs = BTreeSet::new();
    for provider in &projection.providers {
        append_provider_scope_dirs(project_root, provider, &mut dirs);
    }
    dirs
}

fn append_provider_scope_dirs(
    project_root: &Path,
    provider: &RuntimeProvider,
    dirs: &mut BTreeSet<String>,
) {
    for package_root in &provider.package_roots {
        insert_existing_scope_dir(project_root, &project_root.join(package_root), dirs);
    }
    for config_file in &provider.config_files {
        if let Some(parent) = project_root.join(config_file).parent() {
            insert_existing_scope_dir(project_root, parent, dirs);
        }
    }
}

fn insert_existing_scope_dir(project_root: &Path, dir: &Path, dirs: &mut BTreeSet<String>) {
    if !dir.is_dir() {
        return;
    }
    let relative = dir
        .strip_prefix(project_root)
        .ok()
        .and_then(|path| path.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or(".");
    dirs.insert(relative.replace(std::path::MAIN_SEPARATOR, "/"));
}

fn runtime_provider_fingerprint(provider: &RuntimeProvider) -> String {
    [
        format!("registrationDigest={}", provider.registration_digest),
        format!("namespace={}", provider.namespace),
        format!("language={}", provider.language_id),
        format!("provider={}", provider.provider_id),
        "runtimeStatus=server-owned".to_string(),
        format!("packageRoots={}", provider.package_roots.join("\u{1f}")),
        format!("configFiles={}", provider.config_files.join("\u{1f}")),
        format!(
            "sourceExtensions={}",
            provider.source_extensions.join("\u{1f}")
        ),
        format!(
            "searchCapabilities={}",
            serde_json::to_string(&provider.search_capabilities)
                .expect("search-capabilities serialization must be infallible")
        ),
        format!(
            "queryPackDescriptor={}",
            serde_json::to_string(&provider.query_pack_descriptor)
                .expect("query-pack descriptor serialization must be infallible")
        ),
        format!(
            "semanticFactsDescriptor={}",
            serde_json::to_string(&provider.semantic_facts_descriptor)
                .expect("semantic-facts descriptor serialization must be infallible")
        ),
    ]
    .join("\u{1e}")
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderProjectInventoryCapability {
    pub entry_markers: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderDocumentInventoryCapability {
    pub extensions: Vec<String>,
    pub supports_git_candidates: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderSourceInventoryCapabilities {
    pub project_resolution: Option<ProviderProjectInventoryCapability>,
    pub document_resolution: Option<ProviderDocumentInventoryCapability>,
}

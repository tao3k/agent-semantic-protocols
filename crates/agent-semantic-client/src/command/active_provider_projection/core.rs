// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Runtime provider executables derived from the verified active bundle.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use sha2::Digest;
use sha2::Sha256;

const SCHEMA_ID: &str = "agent.semantic-protocols.runtime-active-provider-projection";
const SCHEMA_VERSION: &str = "1";

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ActiveProviderMember {
    language_id: String,
    provider_id: String,
    materialized_path: String,
    artifact_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ActiveProviderProjectionDocument {
    schema_id: String,
    schema_version: String,
    generation: String,
    providers: Vec<ActiveProviderMember>,
}

#[derive(Clone)]
pub(crate) struct RuntimeActiveProviderProjection {
    document: Arc<ActiveProviderProjectionDocument>,
    active_bundle_digest: String,
    execution_binding:
        agent_semantic_artifacts::runtime_artifact_slots::RuntimeArtifactBundleBinding,
}

pub(crate) struct RuntimeProviderLaunch {
    pub(crate) key: String,
    pub(crate) spec: agent_semantic_provider_transport::AspClientServerSpec,
    pub(crate) expected_receipt: agent_semantic_provider_transport::ProviderRuntimeContractReceipt,
}

fn generation(providers: &[ActiveProviderMember]) -> Result<String, String> {
    let stable_members = providers
        .iter()
        .map(|provider| {
            (
                provider.language_id.as_str(),
                provider.provider_id.as_str(),
                provider.artifact_digest.as_str(),
            )
        })
        .collect::<Vec<_>>();
    let bytes = serde_json::to_vec(&stable_members)
        .map_err(|error| format!("encode active provider projection: {error}"))?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

fn validate_serialized_document(document: &ActiveProviderProjectionDocument) -> Result<(), String> {
    if document.schema_id != SCHEMA_ID || document.schema_version != SCHEMA_VERSION {
        return Err("active provider member schema identity mismatch".to_owned());
    }
    if document.generation != generation(&document.providers)? {
        return Err("active provider member generation drift".to_owned());
    }
    let mut languages = std::collections::BTreeSet::new();
    let mut providers = std::collections::BTreeSet::new();
    for provider in &document.providers {
        if !languages.insert(provider.language_id.as_str())
            || !providers.insert(provider.provider_id.as_str())
        {
            return Err("active provider member identities must be unique".to_owned());
        }
        for (field, value) in [
            ("materializedPath", &provider.materialized_path),
            ("artifactDigest", &provider.artifact_digest),
        ] {
            if value.is_empty() {
                return Err(format!("active provider member {field} is empty"));
            }
        }
    }
    Ok(())
}

fn validate_document(document: &ActiveProviderProjectionDocument) -> Result<(), String> {
    validate_serialized_document(document)?;
    for provider in &document.providers {
        let registered = runtime_provider_registration(&provider.language_id)?;
        if registered.provider_id != provider.provider_id {
            return Err(format!(
                "active provider identity drift: languageId={} expectedProviderId={} actualProviderId={}",
                provider.language_id, registered.provider_id, provider.provider_id
            ));
        }
    }
    Ok(())
}

fn runtime_provider_registrations()
-> Result<Vec<agent_semantic_provider_protocol::ProviderRegistrationDocument>, String> {
    agent_semantic_provider_protocol::builtin_provider_registrations()
}

fn runtime_provider_registration(
    language_id: &str,
) -> Result<agent_semantic_provider_protocol::ProviderRegistrationDocument, String> {
    runtime_provider_registrations()?
        .into_iter()
        .find(|registration| registration.language_id == language_id)
        .ok_or_else(|| format!("no Runtime provider capability is registered for `{language_id}`"))
}

fn registration_digest(
    registration: &agent_semantic_provider_protocol::ProviderRegistrationDocument,
) -> Result<String, String> {
    let bytes = serde_json::to_vec(&registration.registration)
        .map_err(|error| format!("encode active provider capability: {error}"))?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

/// Runtime-owned view of the active provider member generation.
impl RuntimeActiveProviderProjection {
    pub(crate) fn generation(&self) -> &str {
        self.active_bundle_digest.as_str()
    }

    pub(crate) fn active_provider_targets(&self) -> Vec<(String, String)> {
        self.document
            .providers
            .iter()
            .map(|provider| (provider.language_id.clone(), provider.provider_id.clone()))
            .collect()
    }

    pub(crate) fn execution_binding(
        &self,
    ) -> &agent_semantic_artifacts::runtime_artifact_slots::RuntimeArtifactBundleBinding {
        &self.execution_binding
    }

    pub(crate) fn runtime_launch(
        &self,
        project_root: &Path,
        language_id: &str,
        register: &agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister,
    ) -> Result<RuntimeProviderLaunch, String> {
        let registration = register.installed_capability(language_id)?;
        let provider = self
            .document
            .providers
            .iter()
            .find(|provider| {
                provider.language_id == language_id
                    && provider.provider_id == registration.provider_id
            })
            .ok_or_else(|| {
                format!(
                    "state=artifact-missing reasonKind=provider-capability-artifact-missing languageId={language_id} providerId={}",
                    registration.provider_id
                )
            })?;
        let operations = serde_json::from_value(
            registration
                .registration_field("runtimeContract")?
                .get("operations")
                .cloned()
                .ok_or_else(|| "provider runtimeContract.operations is required".to_owned())?,
        )
        .map_err(|error| format!("provider runtimeContract.operations are invalid: {error}"))?;
        let registration_digest = registration_digest(&registration)?;
        let expected_receipt =
            agent_semantic_provider_transport::ProviderRuntimeContractReceipt::new(
                provider.provider_id.clone(),
                provider.language_id.clone(),
                provider.artifact_digest.clone(),
                registration_digest.clone(),
                agent_semantic_provider_transport::ProviderRuntimeContractTransport::HttpJson,
                operations,
            )?;
        let mut spec = agent_semantic_provider_transport::AspClientServerSpec::new(
            provider.materialized_path.clone(),
            project_root.to_path_buf(),
        );
        spec.args.push("serve".to_owned());
        spec.env
            .insert("ASP_PROVIDER_ID".to_owned(), provider.provider_id.clone());
        spec.env.insert(
            "ASP_PROVIDER_LANGUAGE_ID".to_owned(),
            provider.language_id.clone(),
        );
        spec.env.insert(
            "ASP_PROVIDER_ARTIFACT_DIGEST".to_owned(),
            provider.artifact_digest.clone(),
        );
        spec.env.insert(
            "ASP_PROVIDER_REGISTRATION_DIGEST".to_owned(),
            registration_digest.clone(),
        );
        spec.env.insert(
            "ASP_PROVIDER_RUNTIME_CONTRACT_DIGEST".to_owned(),
            expected_receipt.contract_digest.clone(),
        );
        spec.env.insert(
            "ASP_PROVIDER_RUNTIME_OPERATIONS_JSON".to_owned(),
            serde_json::to_string(&expected_receipt.operations)
                .map_err(|error| format!("encode provider runtime contract operations: {error}"))?,
        );
        Ok(RuntimeProviderLaunch {
            key: format!(
                "{}:{}:{}:{}:{}",
                self.generation(),
                provider.language_id,
                provider.provider_id,
                provider.artifact_digest,
                registration_digest
            ),
            spec,
            expected_receipt,
        })
    }
}

fn runtime_provider_artifacts_from_active_bundle(
    state_home: &Path,
) -> Result<RuntimeActiveProviderProjection, String> {
    let active = agent_semantic_artifacts::runtime_active_provider_set::
        load_active_runtime_bound_provider_set(state_home)?;
    let mut providers = Vec::new();
    for provider in active.providers {
        let artifact_digest = provider.artifact_digest;
        providers.push(ActiveProviderMember {
            language_id: provider.language_id,
            provider_id: provider.provider_id,
            materialized_path: provider.materialized_path.to_string_lossy().into_owned(),
            artifact_digest,
        });
    }
    providers.sort_by(|left, right| left.language_id.cmp(&right.language_id));
    let document = ActiveProviderProjectionDocument {
        schema_id: SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        generation: generation(&providers)?,
        providers,
    };
    validate_document(&document)?;
    Ok(RuntimeActiveProviderProjection {
        document: Arc::new(document),
        active_bundle_digest: active.runtime_bundle_digest,
        execution_binding: active.execution_binding,
    })
}

pub(crate) async fn load_runtime_active_provider_projection(
    state_home: &Path,
) -> Result<RuntimeActiveProviderProjection, String> {
    let state_home = state_home.to_path_buf();
    tokio::task::spawn_blocking(move || runtime_provider_artifacts_from_active_bundle(&state_home))
        .await
        .map_err(|error| format!("load Runtime active provider projection task failed: {error}"))?
}

pub(crate) fn active_runtime_bundle_digest(state_home: &Path) -> Result<Option<String>, String> {
    runtime_provider_artifacts_from_active_bundle(state_home)
        .map(|artifacts| Some(artifacts.active_bundle_digest))
}

pub(crate) fn runtime_source_index_provider_projection(
    artifacts: &RuntimeActiveProviderProjection,
    register: &agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister,
    required_languages: &BTreeSet<String>,
) -> Result<
    (
        agent_semantic_client_core::RuntimeProviderProjection,
        String,
    ),
    String,
> {
    let registrations = register
        .installed_capabilities()
        .into_iter()
        .filter(|registration| required_languages.contains(&registration.language_id))
        .collect::<Vec<_>>();
    let mut missing = required_languages
        .iter()
        .filter(|language_id| {
            !registrations
                .iter()
                .any(|registration| &registration.language_id == *language_id)
        })
        .map(|language_id| format!("{language_id}:capability"))
        .collect::<Vec<_>>();
    missing.extend(
        registrations
            .iter()
            .filter(|registration| {
                !artifacts.document.providers.iter().any(|artifact| {
                    artifact.language_id == registration.language_id
                        && artifact.provider_id == registration.provider_id
                })
            })
            .map(|registration| format!("{}:artifact", registration.language_id)),
    );
    if !missing.is_empty() {
        missing.sort();
        return Err(format!(
            "state=provider-closure-incomplete reasonKind=workspace-required-provider-closure-incomplete missing={}",
            missing.join(",")
        ));
    }
    runtime_source_index_provider_projection_for_registrations(artifacts, registrations)
}

fn workspace_required_provider_languages_for_paths<'a>(
    _register: &agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister,
    paths: impl IntoIterator<Item = &'a Path>,
) -> Result<BTreeSet<String>, String> {
    let paths = paths.into_iter().collect::<Vec<_>>();
    let mut required = BTreeSet::new();
    for registration in agent_semantic_provider_protocol::builtin_provider_registrations()? {
        let inventory = registration.source_inventory()?;
        let has_entry_marker = inventory
            .project_resolution
            .as_ref()
            .is_some_and(|project| {
                project
                    .entry_markers
                    .iter()
                    .any(|marker| paths.iter().any(|path| *path == Path::new(marker)))
            });
        let has_source = paths.iter().any(|path| {
            let path = path.to_string_lossy();
            inventory
                .source_extensions
                .iter()
                .any(|extension| path.ends_with(extension))
        });
        if has_entry_marker || has_source {
            required.insert(registration.language_id);
        }
    }
    Ok(required)
}

pub(crate) fn workspace_required_provider_languages_for_inventory(
    register: &agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister,
    paths: &[String],
) -> Result<BTreeSet<String>, String> {
    let paths = paths.iter().map(Path::new).collect::<Vec<_>>();
    workspace_required_provider_languages_for_paths(register, paths)
}

pub(crate) fn provider_languages_for_generation_demand(
    register: &agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister,
    paths: &[String],
    provider_target: Option<
        &agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationProviderTarget,
    >,
) -> Result<BTreeSet<String>, String> {
    match provider_target {
        Some(provider_target) => Ok(BTreeSet::from([provider_target.language_id.clone()])),
        None => workspace_required_provider_languages_for_inventory(register, paths),
    }
}

fn runtime_source_index_provider_projection_for_registrations(
    artifacts: &RuntimeActiveProviderProjection,
    registrations: Vec<agent_semantic_provider_protocol::ProviderRegistrationDocument>,
) -> Result<
    (
        agent_semantic_client_core::RuntimeProviderProjection,
        String,
    ),
    String,
> {
    if registrations.is_empty() {
        return Err("state=provider-missing reasonKind=no-active-provider-capability".to_owned());
    }
    let providers = registrations
        .into_iter()
        .map(|registration| {
            let artifact = artifacts
                .document
                .providers
                .iter()
                .find(|artifact| {
                    artifact.language_id == registration.language_id
                        && artifact.provider_id == registration.provider_id
                })
                .ok_or_else(|| {
                    format!(
                        "active provider capability has no bundle member: languageId={} providerId={}",
                        registration.language_id, registration.provider_id
                    )
                })?;
            let inventory = registration.source_inventory()?;
            let search_capabilities = serde_json::from_value(
                registration
                    .registration_field("searchCapabilities")?
                    .clone(),
            )
            .map_err(|error| format!("provider searchCapabilities are invalid: {error}"))?;
            let query_pack_descriptor = serde_json::from_value(
                registration
                    .registration_field("queryPackDescriptor")?
                    .clone(),
            )
            .map_err(|error| format!("provider queryPackDescriptor is invalid: {error}"))?;
            let semantic_facts_descriptor = registration
                .registration
                .get("semanticFactsDescriptor")
                .cloned()
                .map(serde_json::from_value)
                .transpose()
                .map_err(|error| format!("provider semanticFactsDescriptor is invalid: {error}"))?;
            let runtime_operations = serde_json::from_value::<
                Vec<agent_semantic_provider_transport::ProviderRuntimeContractOperation>,
            >(
                registration
                    .registration_field("runtimeContract")?
                    .get("operations")
                    .cloned()
                    .ok_or_else(|| "provider runtimeContract.operations is required".to_owned())?,
            )
            .map_err(|error| format!("provider runtimeContract.operations are invalid: {error}"))?
            .into_iter()
            .map(
                |operation| agent_semantic_client_core::RuntimeProviderOperation {
                    operation: operation.operation,
                    request_schema_id: operation.request_schema.schema_id,
                    response_schema_id: operation.response_schema.schema_id,
                },
            )
            .collect();
            Ok(agent_semantic_client_core::RuntimeProvider {
                registration_digest: registration_digest(&registration)?,
                namespace: registration.namespace()?.to_owned(),
                language_id: agent_semantic_client_core::LanguageId::from(
                    registration.language_id.as_str(),
                ),
                provider_id: agent_semantic_client_core::ProviderId::from(
                    registration.provider_id.as_str(),
                ),
                binary: artifact.materialized_path.clone(),
                package_roots: inventory.package_roots,
                config_files: inventory.config_files,
                source_extensions: inventory.source_extensions,
                source_inventory_capabilities:
                    agent_semantic_client_core::ProviderSourceInventoryCapabilities {
                        project_resolution: inventory.project_resolution.map(|capability| {
                            agent_semantic_client_core::ProviderProjectInventoryCapability {
                                entry_markers: capability.entry_markers,
                            }
                        }),
                        document_resolution: inventory.document_resolution.map(|capability| {
                            agent_semantic_client_core::ProviderDocumentInventoryCapability {
                                extensions: capability.extensions,
                                supports_git_candidates: capability.supports_git_candidates,
                            }
                        }),
                    },
                search_capabilities,
                query_pack_descriptor,
                semantic_facts_descriptor,
                runtime_operations,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let closure_bytes = serde_json::to_vec(
        &providers
            .iter()
            .map(|provider| -> Result<_, String> {
                let artifact = artifacts
                    .document
                    .providers
                    .iter()
                    .find(|artifact| {
                        artifact.language_id == provider.language_id.as_str()
                            && artifact.provider_id == provider.provider_id.as_str()
                    })
                    .ok_or_else(|| {
                        format!(
                            "workspace provider closure lost admitted artifact: languageId={} providerId={}",
                            provider.language_id.as_str(),
                            provider.provider_id.as_str()
                        )
                    })?;
                Ok((
                    provider.language_id.as_str(),
                    provider.provider_id.as_str(),
                    provider.registration_digest.as_str(),
                    artifact.artifact_digest.as_str(),
                ))
            })
            .collect::<Result<Vec<_>, String>>()?,
    )
    .map_err(|error| format!("encode workspace provider closure: {error}"))?;
    let closure_digest = format!("sha256:{:x}", Sha256::digest(&closure_bytes));
    let snapshot = agent_semantic_client_core::RuntimeProviderProjection {
        authority_ref: format!("runtime-provider-register:{closure_digest}"),
        providers,
    };
    Ok((snapshot, closure_digest))
}

#[cfg(test)]
#[path = "../../../tests/unit/command/active_provider_projection.rs"]
mod tests;

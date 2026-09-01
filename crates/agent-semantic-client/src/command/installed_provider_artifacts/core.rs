//! Runtime bootstrap artifacts admitted by immutable provider install receipts.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const SCHEMA_ID: &str = "agent.semantic-protocols.installed-provider-artifacts";
const SCHEMA_VERSION: &str = "1";
const FILE_NAME: &str = "installed-provider-artifacts.json";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InstalledProviderArtifact {
    language_id: String,
    provider_id: String,
    materialized_path: String,
    artifact_digest: String,
    artifact_metadata_digest: String,
    execution_command_digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InstalledProviderArtifactsDocument {
    schema_id: String,
    schema_version: String,
    generation: String,
    providers: Vec<InstalledProviderArtifact>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InstalledProviderArtifactsPublication {
    pub(super) generation: String,
    pub(super) changed_leaf_count: usize,
    pub(super) artifact_write: bool,
    pub(super) elapsed_micros: u64,
}

impl InstalledProviderArtifactsPublication {
    pub(crate) fn generation(&self) -> &str {
        self.generation.as_str()
    }

    pub(crate) fn artifact_write(&self) -> bool {
        self.artifact_write
    }

    pub(crate) fn changed_leaf_count(&self) -> usize {
        self.changed_leaf_count
    }

    pub(crate) fn elapsed_micros(&self) -> u128 {
        self.elapsed_micros.into()
    }
}

#[derive(Clone)]
pub(crate) struct RuntimeProviderArtifacts {
    document: Arc<InstalledProviderArtifactsDocument>,
}

pub(crate) struct RuntimeProviderLaunch {
    pub(crate) key: String,
    pub(crate) spec: agent_semantic_provider_transport::AspClientServerSpec,
    pub(crate) expected_receipt: agent_semantic_provider_transport::ProviderRuntimeContractReceipt,
}

fn document_path(state_home: &Path) -> PathBuf {
    state_home.join("runtime").join(FILE_NAME)
}

fn integrity_ref(digest: &str) -> String {
    if digest.contains(':') {
        digest.to_owned()
    } else {
        format!("blake3-256:{digest}")
    }
}

fn generation(providers: &[InstalledProviderArtifact]) -> Result<String, String> {
    let bytes = serde_json::to_vec(providers)
        .map_err(|error| format!("encode installed provider artifacts: {error}"))?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

fn empty_document() -> Result<InstalledProviderArtifactsDocument, String> {
    Ok(InstalledProviderArtifactsDocument {
        schema_id: SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        generation: generation(&[])?,
        providers: Vec::new(),
    })
}

fn validate_serialized_document(
    document: &InstalledProviderArtifactsDocument,
) -> Result<(), String> {
    if document.schema_id != SCHEMA_ID || document.schema_version != SCHEMA_VERSION {
        return Err("installed provider artifact schema identity mismatch".to_owned());
    }
    if document.generation != generation(&document.providers)? {
        return Err("installed provider artifact generation drift".to_owned());
    }
    let mut languages = std::collections::BTreeSet::new();
    let mut providers = std::collections::BTreeSet::new();
    for provider in &document.providers {
        if !languages.insert(provider.language_id.as_str())
            || !providers.insert(provider.provider_id.as_str())
        {
            return Err("installed provider artifact identities must be unique".to_owned());
        }
        for (field, value) in [
            ("materializedPath", &provider.materialized_path),
            ("artifactDigest", &provider.artifact_digest),
            ("artifactMetadataDigest", &provider.artifact_metadata_digest),
            ("executionCommandDigest", &provider.execution_command_digest),
        ] {
            if value.is_empty() {
                return Err(format!("installed provider artifact {field} is empty"));
            }
        }
    }
    Ok(())
}

fn project_registered_provider_artifacts(
    mut document: InstalledProviderArtifactsDocument,
) -> Result<InstalledProviderArtifactsDocument, String> {
    validate_serialized_document(&document)?;
    let registrations = super::super::provider_install_registry::provider_install_registrations()?
        .into_iter()
        .map(|registration| (registration.language_id, registration.provider_id))
        .collect::<std::collections::BTreeMap<_, _>>();
    for provider in &document.providers {
        if let Some(expected_provider_id) = registrations.get(&provider.language_id)
            && expected_provider_id != &provider.provider_id
        {
            return Err(format!(
                "installed provider identity drift: languageId={} expectedProviderId={} actualProviderId={}",
                provider.language_id, expected_provider_id, provider.provider_id
            ));
        }
    }
    document
        .providers
        .retain(|provider| registrations.contains_key(&provider.language_id));
    document.generation = generation(&document.providers)?;
    validate_serialized_document(&document)?;
    Ok(document)
}

fn validate_document(document: &InstalledProviderArtifactsDocument) -> Result<(), String> {
    validate_serialized_document(document)?;
    for provider in &document.providers {
        let registered = super::super::provider_install_registry::provider_install_registration(
            &provider.language_id,
        )?;
        if registered.provider_id != provider.provider_id {
            return Err(format!(
                "installed provider identity drift: languageId={} expectedProviderId={} actualProviderId={}",
                provider.language_id, registered.provider_id, provider.provider_id
            ));
        }
    }
    Ok(())
}

fn registration_digest(
    registration: &agent_semantic_provider_protocol::ProviderRegistrationDocument,
) -> Result<String, String> {
    let bytes = serde_json::to_vec(&registration.registration)
        .map_err(|error| format!("encode installed provider capability: {error}"))?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

/// Runtime-owned view of the installed provider artifact generation.
impl RuntimeProviderArtifacts {
    pub(crate) fn generation(&self) -> &str {
        &self.document.generation
    }

    pub(crate) fn installed_provider_targets(&self) -> Vec<(String, String)> {
        self.document
            .providers
            .iter()
            .map(|provider| (provider.language_id.clone(), provider.provider_id.clone()))
            .collect()
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
                self.document.generation,
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

pub(crate) async fn load_runtime_provider_artifacts(
    state_home: &Path,
) -> Result<RuntimeProviderArtifacts, String> {
    let path = document_path(state_home);
    let document = match tokio::fs::read(&path).await {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|error| format!("parse {}: {error}", path.display()))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => empty_document()?,
        Err(error) => return Err(format!("read {}: {error}", path.display())),
    };
    let document = project_registered_provider_artifacts(document)?;
    Ok(RuntimeProviderArtifacts {
        document: Arc::new(document),
    })
}

pub(super) fn publish_installed_provider_artifacts(
    state_home: &Path,
    receipts: &[agent_semantic_runtime::ProviderInstallReceipt],
) -> Result<InstalledProviderArtifactsPublication, String> {
    let started = std::time::Instant::now();
    let mut providers = receipts
        .iter()
        .map(|receipt| {
            let expected_provider =
                super::super::provider_install_registry::provider_install_registration(
                    &receipt.language_id,
                )?;
            if expected_provider.provider_id != receipt.provider_id {
                return Err(format!(
                    "provider install receipt identity drift: languageId={} expectedProviderId={} actualProviderId={}",
                    receipt.language_id, expected_provider.provider_id, receipt.provider_id
                ));
            }
            Ok(InstalledProviderArtifact {
                language_id: receipt.language_id.clone(),
                provider_id: receipt.provider_id.clone(),
                materialized_path: receipt.installed_path.to_string_lossy().into_owned(),
                artifact_digest: integrity_ref(&receipt.artifact_digest),
                artifact_metadata_digest: integrity_ref(
                    &receipt.installed_entrypoint_metadata_digest,
                ),
                execution_command_digest: receipt.execution_command_digest.clone(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    providers.sort_by(|left, right| left.language_id.cmp(&right.language_id));
    let document = InstalledProviderArtifactsDocument {
        schema_id: SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        generation: generation(&providers)?,
        providers,
    };
    validate_document(&document)?;
    let path = document_path(state_home);
    let previous = std::fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<InstalledProviderArtifactsDocument>(&bytes).ok())
        .filter(|previous| validate_document(previous).is_ok());
    if previous
        .as_ref()
        .is_some_and(|previous| previous.generation == document.generation)
    {
        return Ok(InstalledProviderArtifactsPublication {
            generation: document.generation,
            changed_leaf_count: 0,
            artifact_write: false,
            elapsed_micros: started.elapsed().as_micros().try_into().unwrap_or(u64::MAX),
        });
    }
    let changed_leaf_count = previous
        .as_ref()
        .map(|previous| {
            document
                .providers
                .iter()
                .filter(|provider| !previous.providers.contains(provider))
                .count()
                + previous
                    .providers
                    .iter()
                    .filter(|provider| !document.providers.contains(provider))
                    .count()
        })
        .unwrap_or(document.providers.len());
    let parent = path.parent().ok_or_else(|| {
        format!(
            "installed provider artifact path has no parent: {}",
            path.display()
        )
    })?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("create {}: {error}", parent.display()))?;
    let temporary = parent.join(format!(".{FILE_NAME}.{}.tmp", std::process::id()));
    let bytes = serde_json::to_vec_pretty(&document)
        .map_err(|error| format!("encode installed provider artifacts: {error}"))?;
    std::fs::write(&temporary, bytes)
        .map_err(|error| format!("write {}: {error}", temporary.display()))?;
    std::fs::rename(&temporary, &path)
        .map_err(|error| format!("publish {}: {error}", path.display()))?;
    Ok(InstalledProviderArtifactsPublication {
        generation: document.generation,
        changed_leaf_count,
        artifact_write: true,
        elapsed_micros: started.elapsed().as_micros().try_into().unwrap_or(u64::MAX),
    })
}

pub(crate) fn publish_current_installed_provider_artifacts(
    state_home: &Path,
) -> Result<InstalledProviderArtifactsPublication, String> {
    let provider_lock_dir = agent_semantic_runtime::provider_receipt_dir(state_home);
    let developer_mode =
        agent_semantic_artifacts::runtime_artifact_catalog::load_runtime_developer_root(
            state_home,
        )?
        .is_some();
    let mut receipts = Vec::new();
    for registration in super::super::provider_install_registry::provider_install_registrations()? {
        let lock_path = provider_lock_dir.join(format!("{}.lock.toml", registration.language_id));
        match std::fs::symlink_metadata(&lock_path) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(format!(
                    "inspect provider install receipt {}: {error}",
                    lock_path.display()
                ));
            }
        }
        let receipt = super::super::provider_install_receipt::read_provider_install_receipt(
            registration.language_id.as_str(),
            &provider_lock_dir,
        )?;
        if receipt.provider_id != registration.provider_id {
            return Err(format!(
                "provider install receipt identity drift: languageId={} expectedProviderId={} actualProviderId={}",
                registration.language_id, registration.provider_id, receipt.provider_id
            ));
        }
        let canonical_artifact = receipt.installed_path.canonicalize().map_err(|error| {
            format!(
                "resolve installed provider artifact {}: {error}",
                receipt.installed_path.display()
            )
        })?;
        let artifact_authority_root = if developer_mode {
            state_home
                .join("runtime/provider-artifacts")
                .join(&receipt.provider_id)
                .join("artifacts")
        } else {
            state_home.join("runtime/artifacts/blake3-256")
        }
        .canonicalize()
        .map_err(|error| {
            format!(
                "resolve provider artifact authority root: languageId={} root={} error={error}",
                receipt.language_id,
                if developer_mode {
                    "developer-provider-staging"
                } else {
                    "managed-release-cas"
                }
            )
        })?;
        if !canonical_artifact.starts_with(&artifact_authority_root) {
            return Err(format!(
                "installed provider artifact escapes mode authority: languageId={} mode={} artifact={} authorityRoot={}",
                receipt.language_id,
                if developer_mode { "dev" } else { "release" },
                canonical_artifact.display(),
                artifact_authority_root.display(),
            ));
        }
        if !super::super::provider_install_receipt::provider_install_receipt_matches_artifact(
            &receipt,
            &receipt.installed_path,
        )? {
            return Err(format!(
                "provider install receipt does not match artifact: languageId={} artifact={}",
                receipt.language_id,
                receipt.installed_path.display()
            ));
        }
        let expected_execution_command_digest =
            agent_semantic_hook::provider_execution_command_digest(
                &[receipt.installed_path.to_string_lossy().into_owned()],
                &receipt.installed_entrypoint_digest,
            )?;
        if receipt.execution_command_digest != expected_execution_command_digest {
            return Err(format!(
                "provider execution command digest drift: languageId={} expected={} actual={}",
                receipt.language_id,
                expected_execution_command_digest,
                receipt.execution_command_digest
            ));
        }
        let mut runtime_receipt = receipt;
        runtime_receipt.installed_path = canonical_artifact;
        runtime_receipt.execution_command_digest =
            agent_semantic_hook::provider_execution_command_digest(
                &[runtime_receipt
                    .installed_path
                    .to_string_lossy()
                    .into_owned()],
                &runtime_receipt.installed_entrypoint_digest,
            )?;
        receipts.push(runtime_receipt);
    }
    publish_installed_provider_artifacts(state_home, &receipts)
}

pub(crate) fn runtime_source_index_provider_projection(
    artifacts: &RuntimeProviderArtifacts,
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
    for language_id in required_languages {
        if !registrations
            .iter()
            .any(|registration| &registration.language_id == language_id)
        {
            return Err(format!(
                "state=provider-missing reasonKind=workspace-required-provider-capability-missing languageId={language_id}"
            ));
        }
    }
    runtime_source_index_provider_projection_for_registrations(artifacts, registrations)
}

pub(crate) fn workspace_required_provider_languages(
    register: &agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister,
    candidates: &agent_semantic_runtime::git::RepositoryCandidateSnapshot,
) -> Result<BTreeSet<String>, String> {
    workspace_required_provider_languages_for_paths(
        register,
        candidates
            .candidates
            .iter()
            .map(|candidate| candidate.path.as_path()),
    )
}

pub(crate) fn provider_language_for_owner_path(
    register: &agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister,
    owner_path: &Path,
) -> Result<String, String> {
    let owner_path = owner_path.to_string_lossy();
    let mut matches = register
        .installed_capabilities()
        .into_iter()
        .filter_map(|registration| {
            let inventory = registration.source_inventory().ok()?;
            inventory
                .source_extensions
                .iter()
                .any(|extension| owner_path.ends_with(extension))
                .then_some(registration.language_id)
        });
    let language_id = matches.next().ok_or_else(|| {
        format!("runtime owner projection has no registered provider: ownerPath={owner_path}")
    })?;
    if matches.next().is_some() {
        return Err(format!(
            "runtime owner projection provider is ambiguous: ownerPath={owner_path}"
        ));
    }
    Ok(language_id)
}

fn workspace_required_provider_languages_for_paths<'a>(
    register: &agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister,
    paths: impl IntoIterator<Item = &'a Path>,
) -> Result<BTreeSet<String>, String> {
    let paths = paths.into_iter().collect::<Vec<_>>();
    let mut required = BTreeSet::new();
    for registration in register.installed_capabilities() {
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

fn runtime_source_index_provider_projection_for_registrations(
    artifacts: &RuntimeProviderArtifacts,
    registrations: Vec<agent_semantic_provider_protocol::ProviderRegistrationDocument>,
) -> Result<
    (
        agent_semantic_client_core::RuntimeProviderProjection,
        String,
    ),
    String,
> {
    if registrations.is_empty() {
        return Err(
            "state=provider-missing reasonKind=no-installed-provider-capability".to_owned(),
        );
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
                        "installed provider capability has no artifact: languageId={} providerId={}",
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
            .map(|provider| {
                (
                    provider.language_id.as_str(),
                    provider.provider_id.as_str(),
                    provider.registration_digest.as_str(),
                    provider.binary.as_str(),
                )
            })
            .collect::<Vec<_>>(),
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
#[path = "../../../tests/unit/command/installed_provider_artifacts.rs"]
mod tests;

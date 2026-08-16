use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

const GLOBAL_PROVIDER_CATALOG_SCHEMA_ID: &str = "asp.global-provider-catalog.v1";
const GLOBAL_PROVIDER_CATALOG_SCHEMA_VERSION: &str = "1";
const GLOBAL_PROVIDER_CATALOG_FILE: &str = "provider-catalog.v1.json";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct GlobalProviderCatalogProvider {
    pub(super) language_id: String,
    pub(super) provider_id: String,
    pub(super) manifest_id: String,
    pub(super) manifest_digest: String,
    pub(super) runtime_contract: agent_semantic_hook::ProviderRuntimeContractDescriptor,
    pub(super) materialized_path: String,
    pub(super) artifact_digest: String,
    pub(super) artifact_metadata_digest: String,
    pub(super) execution_command_digest: String,
    pub(super) exact_parser_identity_digest: String,
    pub(super) argv_prefix: Vec<String>,
    pub(super) provider_registry_digest: String,
    pub(super) query_pack_digest: String,
    pub(super) exact_query_pack_identity_digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct GlobalProviderCatalog {
    schema_id: String,
    schema_version: String,
    catalog_generation: String,
    providers: Vec<GlobalProviderCatalogProvider>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct GlobalProviderCatalogPublication {
    pub(super) catalog_generation: String,
    pub(super) catalog_recovery_reason: Option<String>,
    pub(super) changed_leaf_count: usize,
    pub(super) binary_byte_reads: usize,
    pub(super) catalog_write: bool,
    pub(super) elapsed_micros: u64,
    pub(super) receipt_read_micros: u128,
    pub(super) manifest_digest_micros: u128,
    pub(super) registry_digest_micros: u128,
    pub(super) query_pack_digest_micros: u128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GlobalProviderCatalogReadiness {
    pub(crate) catalog_generation: String,
    pub(crate) provider_count: usize,
    pub(crate) elapsed_micros: u64,
}

fn catalog_path(state_home: &Path) -> PathBuf {
    state_home
        .join("runtime")
        .join(GLOBAL_PROVIDER_CATALOG_FILE)
}

fn canonical_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn generation_digest(providers: &[GlobalProviderCatalogProvider]) -> Result<String, String> {
    let bytes = serde_json::to_vec(providers)
        .map_err(|error| format!("failed to encode Global provider catalog generation: {error}"))?;
    Ok(format!(
        "blake3-256:{}",
        agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1(&bytes)
            .as_str()
    ))
}

fn blake3_integrity_ref(digest: &str) -> String {
    format!("blake3-256:{digest}")
}

fn validate_catalog(catalog: &GlobalProviderCatalog) -> Result<(), String> {
    if catalog.schema_id != GLOBAL_PROVIDER_CATALOG_SCHEMA_ID
        || catalog.schema_version != GLOBAL_PROVIDER_CATALOG_SCHEMA_VERSION
    {
        return Err("Global provider catalog schema identity mismatch".to_owned());
    }
    if catalog.providers.is_empty() {
        return Err("Global provider catalog has no providers".to_owned());
    }
    let expected_generation = generation_digest(&catalog.providers)?;
    if catalog.catalog_generation != expected_generation {
        return Err(format!(
            "Global provider catalog generation drift: expected={expected_generation} actual={}",
            catalog.catalog_generation
        ));
    }
    for provider in &catalog.providers {
        let registered_provider = agent_semantic_hook::registered_provider_id_v1(
            &provider.language_id,
        )
        .ok_or_else(|| {
            format!(
                "Global provider catalog language is not registered: {}",
                provider.language_id
            )
        })?;
        if registered_provider != provider.provider_id {
            return Err(format!(
                "Global provider catalog provider drift: language={} expected={} actual={}",
                provider.language_id, registered_provider, provider.provider_id
            ));
        }
        let registry_digest =
            agent_semantic_hook::registered_language_descriptor_digest(&provider.language_id)
                .ok_or_else(|| {
                    format!(
                        "Global provider catalog language is not registered: {}",
                        provider.language_id
                    )
                })?;
        if provider.provider_registry_digest != registry_digest {
            return Err(format!(
                "Global provider catalog registry drift: language={} expected={} actual={}",
                provider.language_id, registry_digest, provider.provider_registry_digest
            ));
        }
        let exact_identity = agent_semantic_hook::registered_provider_catalog_identities()
            .iter()
            .find(|identity| identity.language_id == provider.language_id)
            .ok_or_else(|| {
                format!(
                    "Global provider catalog exact identity is not registered: {}",
                    provider.language_id
                )
            })?;
        if provider.exact_query_pack_identity_digest
            != exact_identity.exact_query_pack_identity_digest
        {
            return Err(format!(
                "Global provider catalog exact query-pack identity drift: language={}",
                provider.language_id
            ));
        }
        let exact_parser_identity_digest =
            agent_semantic_content_identity::exact_selector_projection_packet::
                derive_parser_identity_digest_v1(
                    &agent_semantic_content_identity::exact_selector_projection_packet::
                        ProjectionPacketProviderIdV1::from(provider.provider_id.as_str()),
                    &agent_semantic_content_identity::exact_selector_projection_packet::
                        ProjectionPacketExecutionCommandDigestV1::from(
                            provider.execution_command_digest.as_str(),
                        ),
                    &agent_semantic_content_identity::exact_selector_projection_packet::
                        ProjectionPacketSemanticRegistryDigestV1::from(
                            provider.provider_registry_digest.as_str(),
                        ),
                )
                .as_str()
                .to_owned();
        if provider.exact_parser_identity_digest != exact_parser_identity_digest {
            return Err(format!(
                "Global provider catalog exact parser identity drift: language={}",
                provider.language_id
            ));
        }
        if provider.argv_prefix.len() != 1 || provider.argv_prefix[0] != provider.materialized_path
        {
            return Err(format!(
                "Global provider catalog argv is not the installed provider artifact: language={} provider={}",
                provider.language_id, provider.provider_id
            ));
        }
    }
    Ok(())
}

fn load_catalog_document_from_disk(
    state_home: &Path,
) -> Result<Arc<GlobalProviderCatalog>, String> {
    let path = catalog_path(state_home);
    let bytes = std::fs::read(&path).map_err(|error| {
        format!(
            "failed to read Global provider catalog {}: {error}",
            path.display()
        )
    })?;
    let catalog: GlobalProviderCatalog = serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "failed to parse Global provider catalog {}: {error}",
            path.display()
        )
    })?;
    validate_catalog(&catalog)?;
    Ok(Arc::new(catalog))
}

fn load_catalog_from_disk(state_home: &Path) -> Result<Arc<GlobalProviderCatalog>, String> {
    let catalog = load_catalog_document_from_disk(state_home)?;
    validate_catalog_artifacts(&catalog)?;
    Ok(catalog)
}

#[derive(Clone)]
pub(crate) struct RuntimeProviderCatalog {
    catalog: Arc<GlobalProviderCatalog>,
}

pub(crate) struct RuntimeProviderLaunch {
    pub(crate) key: String,
    pub(crate) spec: RuntimeProviderLaunchSpec,
    pub(crate) expected_receipt: agent_semantic_provider_transport::ProviderRuntimeContractReceipt,
}

pub(crate) enum RuntimeProviderLaunchSpec {
    Process(agent_semantic_provider_transport::ProviderRuntimeProcessSpec),
    HttpServer(agent_semantic_provider_transport::ProviderHttpServerSpec),
}

impl RuntimeProviderCatalog {
    pub(crate) fn runtime_launch(
        &self,
        project_root: &Path,
        language_id: &str,
    ) -> Result<RuntimeProviderLaunch, String> {
        let provider = self
            .catalog
            .providers
            .iter()
            .find(|provider| provider.language_id == language_id)
            .ok_or_else(|| {
                format!("Runtime search provider is not registered: languageId={language_id}")
            })?;
        let transport = match provider.runtime_contract.transport() {
            agent_semantic_hook::ProviderRuntimeContractTransport::RuntimeIpcV1 => {
                agent_semantic_provider_transport::ProviderRuntimeContractTransport::RuntimeIpcV1
            }
            agent_semantic_hook::ProviderRuntimeContractTransport::HttpJsonV1 => {
                agent_semantic_provider_transport::ProviderRuntimeContractTransport::HttpJsonV1
            }
            agent_semantic_hook::ProviderRuntimeContractTransport::InProcessV1 => {
                return Err(format!(
                    "provider-runtime-not-process-owned: languageId={language_id} providerId={}",
                    provider.provider_id
                ));
            }
        };
        let (program, prefix_args) = provider.argv_prefix.split_first().ok_or_else(|| {
            format!(
                "runtime provider command is empty: languageId={language_id} providerId={}",
                provider.provider_id
            )
        })?;
        let operations = provider
            .runtime_contract
            .operations()
            .iter()
            .map(
                |operation| agent_semantic_provider_transport::ProviderRuntimeContractOperation {
                    operation: operation.operation().to_owned(),
                    request_schema_id: operation.request_schema_id().to_owned(),
                    response_schema_id: operation.response_schema_id().to_owned(),
                },
            )
            .collect();
        let expected_receipt =
            agent_semantic_provider_transport::ProviderRuntimeContractReceipt::new(
                provider.provider_id.clone(),
                provider.language_id.clone(),
                provider.artifact_digest.clone(),
                provider.manifest_digest.clone(),
                transport.clone(),
                operations,
            )?;
        let server_descriptor = provider.runtime_contract.server();
        let launch_args = match transport {
            agent_semantic_provider_transport::ProviderRuntimeContractTransport::HttpJsonV1 => {
                server_descriptor
                    .ok_or_else(|| "provider HTTP server descriptor is absent".to_owned())?
                    .command()
                    .to_vec()
            }
            _ => vec!["runtime".to_owned(), "serve".to_owned()],
        };
        let mut env = std::collections::BTreeMap::new();
        env.insert(
            "ASP_PROVIDER_ARTIFACT_DIGEST".to_owned(),
            provider.artifact_digest.clone(),
        );
        env.insert(
            "ASP_PROVIDER_MANIFEST_DIGEST".to_owned(),
            provider.manifest_digest.clone(),
        );
        env.insert(
            "ASP_PROVIDER_RUNTIME_CONTRACT_DIGEST".to_owned(),
            expected_receipt.contract_digest.clone(),
        );
        let spec = match transport {
            agent_semantic_provider_transport::ProviderRuntimeContractTransport::HttpJsonV1 => {
                let mut spec = agent_semantic_provider_transport::ProviderHttpServerSpec::new(
                    program.clone(),
                    project_root.to_path_buf(),
                );
                let server = server_descriptor
                    .ok_or_else(|| "provider HTTP server descriptor is absent".to_owned())?;
                spec.args = prefix_args.iter().cloned().chain(launch_args).collect();
                spec.env = env;
                spec.health_path = server.health_path().to_owned();
                spec.request_path = server.request_path().to_owned();
                spec.shutdown_path = server.shutdown_path().to_owned();
                RuntimeProviderLaunchSpec::HttpServer(spec)
            }
            _ => {
                let mut spec = agent_semantic_provider_transport::ProviderRuntimeProcessSpec::new(
                    program.clone(),
                    project_root.to_path_buf(),
                );
                spec.args = prefix_args.iter().cloned().chain(launch_args).collect();
                spec.env = env;
                RuntimeProviderLaunchSpec::Process(spec)
            }
        };
        Ok(RuntimeProviderLaunch {
            key: format!(
                "{}:{}:{}",
                provider.language_id,
                provider.provider_id,
                project_root.to_string_lossy()
            ),
            spec,
            expected_receipt,
        })
    }
}

pub(crate) async fn load_runtime_provider_catalog(
    state_home: &Path,
) -> Result<RuntimeProviderCatalog, String> {
    let path = catalog_path(state_home);
    let bytes = tokio::fs::read(&path).await.map_err(|error| {
        format!(
            "failed to read Global provider catalog {}: {error}",
            path.display()
        )
    })?;
    let catalog: GlobalProviderCatalog = serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "failed to parse Global provider catalog {}: {error}",
            path.display()
        )
    })?;
    validate_catalog(&catalog)?;
    let catalog = Arc::new(catalog);
    let artifact_catalog = Arc::clone(&catalog);
    tokio::task::spawn_blocking(move || validate_catalog_artifacts(&artifact_catalog))
        .await
        .map_err(|error| {
            format!("Global provider catalog artifact validation task failed: {error}")
        })??;
    Ok(RuntimeProviderCatalog { catalog })
}

fn validate_catalog_artifacts(catalog: &GlobalProviderCatalog) -> Result<(), String> {
    for provider in &catalog.providers {
        let path = Path::new(&provider.materialized_path);
        let artifact_digest = blake3_integrity_ref(
            &agent_semantic_content_identity::file_content_digest_v1(path)?,
        );
        if artifact_digest != provider.artifact_digest {
            return Err(format!(
                "Global provider catalog artifact digest drift: languageId={} providerId={}",
                provider.language_id, provider.provider_id
            ));
        }
        let metadata_digest = blake3_integrity_ref(
            &agent_semantic_content_identity::file_artifact_metadata_digest_v1(path)?,
        );
        if metadata_digest != provider.artifact_metadata_digest {
            return Err(format!(
                "Global provider catalog artifact metadata drift: languageId={} providerId={}",
                provider.language_id, provider.provider_id
            ));
        }
    }
    Ok(())
}

pub(crate) fn read_global_provider_catalog_readiness(
    state_home: &Path,
) -> Result<GlobalProviderCatalogReadiness, String> {
    let started_at = std::time::Instant::now();
    let catalog = load_catalog_from_disk(state_home)?;
    Ok(GlobalProviderCatalogReadiness {
        catalog_generation: catalog.catalog_generation.clone(),
        provider_count: catalog.providers.len(),
        elapsed_micros: started_at
            .elapsed()
            .as_micros()
            .try_into()
            .unwrap_or(u64::MAX),
    })
}

pub(crate) fn empty_global_provider_catalog_readiness()
-> Result<GlobalProviderCatalogReadiness, String> {
    Ok(GlobalProviderCatalogReadiness {
        catalog_generation: generation_digest(&[])?,
        provider_count: 0,
        elapsed_micros: 0,
    })
}

pub(crate) fn read_runtime_provider_catalog_readiness(
    state_home: &Path,
) -> Result<GlobalProviderCatalogReadiness, String> {
    let path = catalog_path(state_home);
    match std::fs::metadata(&path) {
        Ok(_) => read_global_provider_catalog_readiness(state_home),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            empty_global_provider_catalog_readiness()
        }
        Err(error) => Err(format!(
            "failed to inspect Global provider catalog {}: {error}",
            path.display()
        )),
    }
}

pub(crate) fn runtime_provider_registry_snapshot(
    project_root: &Path,
    runtime_catalog: &RuntimeProviderCatalog,
) -> Result<(agent_semantic_client_core::ProviderRegistrySnapshot, String), String> {
    let catalog = &runtime_catalog.catalog;
    let mut snapshot = agent_semantic_client_core::ProviderRegistrySnapshot::load(project_root)?;
    for provider in &mut snapshot.providers {
        let catalog_provider = catalog
            .providers
            .iter()
            .find(|candidate| {
                candidate.language_id == provider.language_id.as_str()
                    && candidate.provider_id == provider.provider_id.as_str()
            })
            .ok_or_else(|| {
                format!(
                    "runtime provider catalog omitted activated provider: languageId={} providerId={}",
                    provider.language_id, provider.provider_id
                )
            })?;
        provider.manifest_digest = catalog_provider.manifest_digest.clone();
        provider.execution_command_digest = catalog_provider.execution_command_digest.clone();
        provider.provider_command_prefix = catalog_provider.argv_prefix.clone();
        provider.runtime_command_argv = Some(catalog_provider.argv_prefix.clone());
        provider.runtime_profile_status =
            Some(agent_semantic_client_core::RuntimeProfileStatus::Available);
    }
    Ok((snapshot, catalog.catalog_generation.clone()))
}

pub(super) fn publish_global_provider_catalog(
    state_home: &Path,
    receipts: &[super::install_provider_reconcile::ProviderInstallReceipt],
) -> Result<GlobalProviderCatalogPublication, String> {
    let started_at = std::time::Instant::now();
    let receipt_read_micros = 0;
    let manifest_digest_micros = 0;
    let mut registry_digest_micros = 0;
    let query_pack_digest_micros = 0;
    let phase = std::time::Instant::now();
    let catalog_identities = agent_semantic_hook::registered_provider_catalog_identities();
    registry_digest_micros += phase.elapsed().as_micros();
    let manifests = agent_semantic_hook::schema_registry_provider_manifests()
        .into_iter()
        .map(|manifest| {
            let kind =
                agent_semantic_hook::registered_provider_kind(manifest.language_id().as_str())?;
            Ok((kind, manifest))
        })
        .collect::<Result<Vec<_>, String>>()?
        .into_iter()
        .filter_map(|(kind, manifest)| {
            (kind == agent_semantic_hook::RegisteredProviderKind::ProgrammingLanguage)
                .then_some(manifest)
        })
        .filter(|manifest| {
            receipts.iter().any(|receipt| {
                receipt.language_id == manifest.language_id().as_str()
                    || receipt
                        .installed_path
                        .file_name()
                        .and_then(|name| name.to_str())
                        == Some(manifest.binary())
            })
        })
        .collect::<Vec<_>>();
    let path = catalog_path(state_home);
    let (active_catalog, mut catalog_recovery_reason) = if path.is_file() {
        match load_catalog_document_from_disk(state_home) {
            Ok(active) => (Some(active), None),
            Err(error) => (None, Some(format!("malformed-active-catalog: {error}"))),
        }
    } else {
        (None, Some("missing-active-catalog".to_owned()))
    };
    let active_generation = active_catalog.as_ref().and_then(|active| {
        active_catalog_matches_receipts(active, &manifests, receipts, catalog_identities)
            .then(|| active.catalog_generation.clone())
    });
    if active_catalog.is_some() && active_generation.is_none() {
        catalog_recovery_reason = Some("stale-active-catalog".to_owned());
    }
    if let Some(catalog_generation) = active_generation {
        return Ok(GlobalProviderCatalogPublication {
            catalog_generation,
            catalog_recovery_reason: None,
            changed_leaf_count: 0,
            binary_byte_reads: 0,
            catalog_write: false,
            elapsed_micros: started_at
                .elapsed()
                .as_micros()
                .try_into()
                .unwrap_or(u64::MAX),
            receipt_read_micros,
            manifest_digest_micros,
            registry_digest_micros,
            query_pack_digest_micros,
        });
    }
    let mut providers = manifests
        .iter()
        .map(|manifest| {
            let receipt = receipts
                .iter()
                .find(|receipt| receipt.language_id == manifest.language_id().as_str())
                .or_else(|| {
                    receipts.iter().find(|receipt| {
                        receipt
                            .installed_path
                            .file_name()
                            .and_then(|name| name.to_str())
                            == Some(manifest.binary())
                    })
                })
                .ok_or_else(|| {
                    format!(
                        "Global provider catalog lacks install receipt for language={} binary={}",
                        manifest.language_id(),
                        manifest.binary()
                    )
                })?;
            if receipt.language_id == manifest.language_id().as_str()
                && receipt.provider_id != manifest.provider_id().as_str()
            {
                return Err(format!(
                    "provider install receipt identity mismatch: language={} expectedProvider={} actualProvider={}",
                    receipt.language_id,
                    manifest.provider_id(),
                    receipt.provider_id
                ));
            }
            let provider_path = canonical_path(&receipt.installed_path);
            if provider_path.file_name().and_then(|name| name.to_str()) != Some(manifest.binary()) {
                return Err(format!(
                    "provider install receipt binary mismatch: language={} expectedBinary={} actualPath={}",
                    manifest.language_id(),
                    manifest.binary(),
                    provider_path.display()
                ));
            }
            let artifact_digest =
                blake3_integrity_ref(&receipt.installed_entrypoint_digest);
            let command_prefix = vec![provider_path.to_string_lossy().to_string()];
            let identity = catalog_identities
                .iter()
                .find(|identity| identity.language_id == manifest.language_id().as_str())
                .ok_or_else(|| {
                    format!(
                        "Global provider catalog language identity is not registered: {}",
                        manifest.language_id()
                    )
                })?;
            let manifest_digest = identity.manifest_digest.clone();
            let provider_registry_digest = identity.provider_registry_digest.clone();
            let query_pack_digest = identity.query_pack_digest.clone();
            let exact_parser_identity_digest =
                agent_semantic_content_identity::exact_selector_projection_packet::
                    derive_parser_identity_digest_v1(
                        &agent_semantic_content_identity::exact_selector_projection_packet::
                        ProjectionPacketProviderIdV1::from(manifest.provider_id().as_str()),
                        &agent_semantic_content_identity::exact_selector_projection_packet::
                            ProjectionPacketExecutionCommandDigestV1::from(
                                receipt.execution_command_digest.as_str(),
                            ),
                        &agent_semantic_content_identity::exact_selector_projection_packet::
                            ProjectionPacketSemanticRegistryDigestV1::from(
                                provider_registry_digest.as_str(),
                            ),
                    )
                    .as_str()
                    .to_owned();
            Ok(GlobalProviderCatalogProvider {
                language_id: manifest.language_id().to_string(),
                provider_id: manifest.provider_id().to_string(),
    manifest_id: manifest.manifest_id().to_owned(),
            manifest_digest,
    runtime_contract: manifest.runtime_contract().clone(),
    materialized_path: provider_path.to_string_lossy().to_string(),
                artifact_digest,
                artifact_metadata_digest: blake3_integrity_ref(
                    &receipt.installed_entrypoint_metadata_digest,
                ),
                execution_command_digest: receipt.execution_command_digest.clone(),
                exact_parser_identity_digest,
                argv_prefix: command_prefix,
                provider_registry_digest,
                query_pack_digest,
                exact_query_pack_identity_digest: identity
                    .exact_query_pack_identity_digest
                    .clone(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    providers.sort_by(|left, right| {
        (&left.language_id, &left.provider_id).cmp(&(&right.language_id, &right.provider_id))
    });
    let catalog = GlobalProviderCatalog {
        schema_id: GLOBAL_PROVIDER_CATALOG_SCHEMA_ID.to_owned(),
        schema_version: GLOBAL_PROVIDER_CATALOG_SCHEMA_VERSION.to_owned(),
        catalog_generation: generation_digest(&providers)?,
        providers,
    };
    if active_catalog
        .as_ref()
        .is_some_and(|active| active.catalog_generation == catalog.catalog_generation)
    {
        return Ok(GlobalProviderCatalogPublication {
            catalog_generation: catalog.catalog_generation,
            catalog_recovery_reason: None,
            changed_leaf_count: 0,
            binary_byte_reads: 0,
            catalog_write: false,
            elapsed_micros: started_at
                .elapsed()
                .as_micros()
                .try_into()
                .unwrap_or(u64::MAX),
            receipt_read_micros,
            manifest_digest_micros,
            registry_digest_micros,
            query_pack_digest_micros,
        });
    }
    validate_catalog(&catalog)?;
    let path = catalog_path(state_home);
    let previous = if path.exists() {
        let bytes = std::fs::read(&path).map_err(|error| {
            format!(
                "failed to read Global provider catalog {}: {error}",
                path.display()
            )
        })?;
        serde_json::from_slice::<GlobalProviderCatalog>(&bytes)
            .ok()
            .filter(|previous| validate_catalog(previous).is_ok())
    } else {
        None
    };
    let changed_leaf_count = previous
        .as_ref()
        .map(|previous| {
            catalog
                .providers
                .iter()
                .filter(|provider| {
                    previous
                        .providers
                        .iter()
                        .find(|candidate| candidate.language_id == provider.language_id)
                        != Some(*provider)
                })
                .count()
                + previous
                    .providers
                    .iter()
                    .filter(|provider| {
                        !catalog
                            .providers
                            .iter()
                            .any(|candidate| candidate.language_id == provider.language_id)
                    })
                    .count()
        })
        .unwrap_or(catalog.providers.len());
    if previous
        .as_ref()
        .is_some_and(|previous| previous.catalog_generation == catalog.catalog_generation)
    {
        return Ok(GlobalProviderCatalogPublication {
            catalog_generation: catalog.catalog_generation,
            catalog_recovery_reason: None,
            changed_leaf_count: 0,
            binary_byte_reads: 0,
            catalog_write: false,
            elapsed_micros: started_at
                .elapsed()
                .as_micros()
                .try_into()
                .unwrap_or(u64::MAX),
            receipt_read_micros,
            manifest_digest_micros,
            registry_digest_micros,
            query_pack_digest_micros,
        });
    }
    let parent = path.parent().ok_or_else(|| {
        format!(
            "Global provider catalog path has no parent: {}",
            path.display()
        )
    })?;
    std::fs::create_dir_all(parent).map_err(|error| {
        format!(
            "failed to create Global provider catalog directory {}: {error}",
            parent.display()
        )
    })?;
    let temporary = parent.join(format!(
        ".{GLOBAL_PROVIDER_CATALOG_FILE}.{}.tmp",
        std::process::id()
    ));
    let bytes = serde_json::to_vec_pretty(&catalog)
        .map_err(|error| format!("failed to encode Global provider catalog: {error}"))?;
    std::fs::write(&temporary, bytes).map_err(|error| {
        format!(
            "failed to write Global provider catalog staging file {}: {error}",
            temporary.display()
        )
    })?;
    std::fs::rename(&temporary, &path).map_err(|error| {
        format!(
            "failed to publish Global provider catalog {}: {error}",
            path.display()
        )
    })?;
    Ok(GlobalProviderCatalogPublication {
        catalog_generation: catalog.catalog_generation,
        catalog_recovery_reason,
        changed_leaf_count,
        binary_byte_reads: 0,
        catalog_write: true,
        elapsed_micros: started_at
            .elapsed()
            .as_micros()
            .try_into()
            .unwrap_or(u64::MAX),
        receipt_read_micros,
        manifest_digest_micros,
        registry_digest_micros,
        query_pack_digest_micros,
    })
}

pub(super) fn active_catalog_matches_receipts(
    active: &GlobalProviderCatalog,
    manifests: &[agent_semantic_hook::ProviderManifest],
    receipts: &[super::install_provider_reconcile::ProviderInstallReceipt],
    catalog_identities: &[agent_semantic_hook::RegisteredProviderCatalogIdentity],
) -> bool {
    active.providers.len() == manifests.len()
        && manifests.iter().all(|manifest| {
            let Some(provider) = active
                .providers
                .iter()
                .find(|provider| provider.language_id == manifest.language_id().as_str())
            else {
                return false;
            };
            let Some(receipt) = receipts
                .iter()
                .find(|receipt| receipt.language_id == manifest.language_id().as_str())
                .or_else(|| {
                    receipts.iter().find(|receipt| {
                        receipt
                            .installed_path
                            .file_name()
                            .and_then(|name| name.to_str())
                            == Some(manifest.binary())
                    })
                })
            else {
                return false;
            };
            let Some(identity) = catalog_identities
                .iter()
                .find(|identity| identity.language_id == manifest.language_id().as_str())
            else {
                return false;
            };
            provider.provider_id == manifest.provider_id().as_str()
                && provider.manifest_digest == identity.manifest_digest
                && provider.provider_registry_digest == identity.provider_registry_digest
                && provider.query_pack_digest == identity.query_pack_digest
                && provider.exact_query_pack_identity_digest
                    == identity.exact_query_pack_identity_digest
                && provider.artifact_digest
                    == blake3_integrity_ref(&receipt.installed_entrypoint_digest)
                && provider.artifact_metadata_digest
                    == blake3_integrity_ref(&receipt.installed_entrypoint_metadata_digest)
                && provider.execution_command_digest == receipt.execution_command_digest
        })
}

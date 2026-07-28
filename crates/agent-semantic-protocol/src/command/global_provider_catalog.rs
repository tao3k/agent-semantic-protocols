use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};

use agent_semantic_content_identity::active_artifact_merkle_v1::ActiveArtifactKindV1;
use serde::{Deserialize, Serialize};

const GLOBAL_PROVIDER_CATALOG_SCHEMA_ID: &str = "asp.global-provider-catalog.v1";
const GLOBAL_PROVIDER_CATALOG_SCHEMA_VERSION: &str = "1";
const GLOBAL_PROVIDER_CATALOG_FILE: &str = "provider-catalog.v1.json";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct GlobalProviderCatalogProviderV1 {
    pub(super) language_id: String,
    pub(super) provider_id: String,
    pub(super) manifest_id: String,
    pub(super) manifest_digest: String,
    pub(super) materialized_path: String,
    pub(super) artifact_digest: String,
    pub(super) artifact_metadata_digest: String,
    pub(super) execution_command_digest: String,
    pub(super) semantic_registry_digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GlobalProviderCatalogV1 {
    schema_id: String,
    schema_version: String,
    catalog_generation: String,
    providers: Vec<GlobalProviderCatalogProviderV1>,
}

fn catalog_path() -> Result<PathBuf, String> {
    Ok(agent_semantic_runtime::state_core::resolve_state_home()?
        .join("runtime")
        .join(GLOBAL_PROVIDER_CATALOG_FILE))
}

fn canonical_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn generation_digest(
    providers: &[GlobalProviderCatalogProviderV1],
) -> Result<String, String> {
    let bytes = serde_json::to_vec(providers)
        .map_err(|error| format!("failed to encode Global provider catalog generation: {error}"))?;
    Ok(
        agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1(&bytes)
            .as_str()
            .to_owned(),
    )
}

fn validate_catalog(catalog: &GlobalProviderCatalogV1) -> Result<(), String> {
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
    let registry_digest = agent_semantic_hook::semantic_registry_digest();
    for provider in &catalog.providers {
        let registered_provider =
            agent_semantic_hook::registered_provider_id_v1(&provider.language_id).ok_or_else(
                || {
                    format!(
                        "Global provider catalog language is not registered: {}",
                        provider.language_id
                    )
                },
            )?;
        if registered_provider != provider.provider_id {
            return Err(format!(
                "Global provider catalog provider drift: language={} expected={} actual={}",
                provider.language_id, registered_provider, provider.provider_id
            ));
        }
        if provider.semantic_registry_digest != registry_digest {
            return Err(format!(
                "Global provider catalog registry drift: language={} expected={} actual={}",
                provider.language_id, registry_digest, provider.semantic_registry_digest
            ));
        }
        let materialized_path = Path::new(&provider.materialized_path);
        let artifact_digest =
            agent_semantic_content_identity::file_content_digest_v1(materialized_path)?;
        if artifact_digest != provider.artifact_digest {
            return Err(format!(
                "Global provider catalog artifact digest drift: language={} path={}",
                provider.language_id,
                materialized_path.display()
            ));
        }
        let artifact_metadata_digest =
            agent_semantic_content_identity::file_artifact_metadata_digest_v1(materialized_path)?
                .to_string();
        if artifact_metadata_digest != provider.artifact_metadata_digest {
            return Err(format!(
                "Global provider catalog artifact metadata drift: language={} path={}",
                provider.language_id,
                materialized_path.display()
            ));
        }
    }
    Ok(())
}

fn active_catalog() -> &'static RwLock<Option<Arc<GlobalProviderCatalogV1>>> {
    static ACTIVE: OnceLock<RwLock<Option<Arc<GlobalProviderCatalogV1>>>> = OnceLock::new();
    ACTIVE.get_or_init(Default::default)
}

fn load_catalog_from_disk() -> Result<Arc<GlobalProviderCatalogV1>, String> {
    let path = catalog_path()?;
    let bytes = std::fs::read(&path)
        .map_err(|error| format!("failed to read Global provider catalog {}: {error}", path.display()))?;
    let catalog: GlobalProviderCatalogV1 = serde_json::from_slice(&bytes)
        .map_err(|error| format!("failed to parse Global provider catalog {}: {error}", path.display()))?;
    validate_catalog(&catalog)?;
    Ok(Arc::new(catalog))
}

fn load_once_catalog() -> Result<Arc<GlobalProviderCatalogV1>, String> {
    if let Some(catalog) = active_catalog()
        .read()
        .map_err(|_| "Global provider catalog read guard is poisoned".to_owned())?
        .as_ref()
    {
        return Ok(Arc::clone(catalog));
    }
    let catalog = load_catalog_from_disk()?;
    let mut active = active_catalog()
        .write()
        .map_err(|_| "Global provider catalog write guard is poisoned".to_owned())?;
    let catalog = active.get_or_insert_with(|| Arc::clone(&catalog));
    Ok(Arc::clone(catalog))
}

pub(super) fn global_provider_for_language_v1(
    language_id: &str,
) -> Result<GlobalProviderCatalogProviderV1, String> {
    let catalog = load_once_catalog()?;
    catalog
        .providers
        .iter()
        .find(|provider| provider.language_id == language_id)
        .cloned()
        .ok_or_else(|| {
            format!(
                "Global provider catalog has no provider for language {language_id}; run the explicit Global plugin reconciliation"
            )
        })
}

pub(super) fn publish_global_provider_catalog_v1(
    activation: &agent_semantic_hook::HookActivation,
    artifacts: &[agent_semantic_hook::ActiveAspArtifactInput],
) -> Result<String, String> {
    let registry_digest = agent_semantic_hook::semantic_registry_digest();
    let mut providers = activation
        .providers
        .iter()
        .map(|provider| {
            let provider_path = canonical_path(Path::new(&provider.binary));
            let artifact = artifacts
                .iter()
                .filter(|artifact| artifact.artifact_kind == ActiveArtifactKindV1::ProviderBinary)
                .find(|artifact| canonical_path(&artifact.materialized_path) == provider_path)
                .ok_or_else(|| {
                    format!(
                        "Global provider catalog is missing installed artifact: language={} provider={} path={}",
                        provider.language_id,
                        provider.provider_id,
                        provider_path.display()
                    )
                })?;
            Ok(GlobalProviderCatalogProviderV1 {
                language_id: provider.language_id.to_string(),
                provider_id: provider.provider_id.to_string(),
                manifest_id: provider.manifest_id.clone(),
                manifest_digest: provider.manifest_digest.clone(),
                materialized_path: provider_path.to_string_lossy().to_string(),
                artifact_digest: artifact.artifact_digest.clone(),
                artifact_metadata_digest:
                    agent_semantic_content_identity::file_artifact_metadata_digest_v1(
                        &provider_path,
                    )?
                    .to_string(),
                execution_command_digest: provider.execution_command_digest.clone(),
                semantic_registry_digest: registry_digest.clone(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    providers.sort_by(|left, right| {
        (&left.language_id, &left.provider_id).cmp(&(&right.language_id, &right.provider_id))
    });
    let catalog = GlobalProviderCatalogV1 {
        schema_id: GLOBAL_PROVIDER_CATALOG_SCHEMA_ID.to_owned(),
        schema_version: GLOBAL_PROVIDER_CATALOG_SCHEMA_VERSION.to_owned(),
        catalog_generation: generation_digest(&providers)?,
        providers,
    };
    validate_catalog(&catalog)?;
    let path = catalog_path()?;
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
    *active_catalog()
        .write()
        .map_err(|_| "Global provider catalog write guard is poisoned".to_owned())? =
        Some(Arc::new(catalog.clone()));
    Ok(catalog.catalog_generation)
}

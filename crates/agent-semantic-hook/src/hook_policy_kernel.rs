use agent_semantic_config::HookClientLanguageProviderConfig;
use agent_semantic_content_identity::exact_selector_merkle::canonical_content_digest_v1;
use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, OnceLock, RwLock};

pub(crate) const HOOK_POLICY_KERNEL_VERSION: &str = "hook-enforcement-kernel-v1";

#[derive(Clone, Debug)]
pub(crate) struct HookPolicySnapshot {
    pub kernel_version: &'static str,
    pub generation_digest: String,
    pub provider_projections:
        Vec<crate::protocol_activation::protocol_activation_manifest::HookProviderProjection>,
}

static ACTIVE_POLICY_SNAPSHOTS: OnceLock<RwLock<HashMap<String, Arc<HookPolicySnapshot>>>> =
    OnceLock::new();

fn snapshot_store() -> &'static RwLock<HashMap<String, Arc<HookPolicySnapshot>>> {
    ACTIVE_POLICY_SNAPSHOTS.get_or_init(|| RwLock::new(HashMap::new()))
}

fn compile_language_provider_snapshot(
    language_providers: &[HookClientLanguageProviderConfig],
) -> Result<HookPolicySnapshot, String> {
    if language_providers.is_empty() {
        return Err("hook policy snapshot requires at least one language provider".to_owned());
    }
    let mut providers = language_providers.to_vec();
    providers.sort_by(|left, right| {
        (&left.language_id, &left.provider_id).cmp(&(&right.language_id, &right.provider_id))
    });
    let mut identities = BTreeSet::new();
    for provider in &providers {
        let identity = (provider.language_id.as_str(), provider.provider_id.as_str());
        if provider.language_id.trim().is_empty()
            || provider.provider_id.trim().is_empty()
            || provider.manifest_digest.trim().is_empty()
        {
            return Err("hook policy snapshot provider identity is incomplete".to_owned());
        }
        if !identities.insert(identity) {
            return Err(format!(
                "hook policy snapshot contains duplicate provider `{}/{}`",
                provider.language_id, provider.provider_id
            ));
        }
        if provider.source_extensions.is_empty() {
            return Err(format!(
                "hook policy snapshot provider `{}/{}` has no source extensions",
                provider.language_id, provider.provider_id
            ));
        }
        let mut extensions = BTreeSet::new();
        for extension in &provider.source_extensions {
            if !extension.starts_with('.') || extension.len() < 2 {
                return Err(format!(
                    "hook policy snapshot provider `{}/{}` has invalid extension `{extension}`",
                    provider.language_id, provider.provider_id
                ));
            }
            if !extensions.insert(extension.as_str()) {
                return Err(format!(
                    "hook policy snapshot provider `{}/{}` repeats extension `{extension}`",
                    provider.language_id, provider.provider_id
                ));
            }
        }
    }
    let canonical = serde_json::to_vec(&providers)
        .map_err(|error| format!("serialize hook policy language providers: {error}"))?;
    let digest = canonical_content_digest_v1(
        b"agent.semantic-protocols.hook-policy-language-providers.v1",
        &[HOOK_POLICY_KERNEL_VERSION.as_bytes(), canonical.as_slice()],
    );
    let manifests = crate::builtin_provider_manifests();
    let provider_projections = providers
        .iter()
        .map(|provider| {
            let manifest = manifests
                .iter()
                .find(|manifest| {
                    manifest.language_id().as_str() == provider.language_id
                        && manifest.provider_id().as_str() == provider.provider_id
                })
                .ok_or_else(|| {
                    format!(
                        "hook policy snapshot has no registered provider `{}/{}`",
                        provider.language_id, provider.provider_id
                    )
                })?;
            let registered_digest =
                crate::provider_manifest_digest(manifest).map_err(|error| error.to_string())?;
            if registered_digest != provider.manifest_digest {
                return Err(format!(
                    "hook policy snapshot manifest drift for `{}/{}`: configured {} registered {}",
                    provider.language_id,
                    provider.provider_id,
                    provider.manifest_digest,
                    registered_digest
                ));
            }
            let config_files = manifest
                .project_resolution()
                .map(|descriptor| descriptor.entry_markers.clone())
                .unwrap_or_default();
            Ok(
                crate::protocol_activation::protocol_activation_manifest::HookProviderProjection {
                    language_id: manifest.language_id().clone(),
                    provider_id: manifest.provider_id().clone(),
                    binary: manifest.binary().to_owned(),
                    provider_command_prefix: Vec::new(),
                    package_roots: vec![".".to_owned()],
                    source_extensions: provider.source_extensions.clone(),
                    config_files,
                    policy: manifest.policy().clone(),
                    routes: crate::materialize_provider_routes(manifest)
                        .map_err(|error| error.to_string())?,
                },
            )
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(HookPolicySnapshot {
        kernel_version: HOOK_POLICY_KERNEL_VERSION,
        generation_digest: format!("blake3-256:{}", digest.as_str()),
        provider_projections,
    })
}

pub(crate) fn publish_language_provider_snapshot(
    project_root: &str,
    language_providers: &[HookClientLanguageProviderConfig],
) -> Result<Arc<HookPolicySnapshot>, String> {
    let candidate = Arc::new(compile_language_provider_snapshot(language_providers)?);
    let mut snapshots = snapshot_store()
        .write()
        .map_err(|_| "hook policy snapshot store is poisoned".to_owned())?;
    if let Some(active) = snapshots.get(project_root)
        && active.generation_digest == candidate.generation_digest
    {
        return Ok(Arc::clone(active));
    }
    snapshots.insert(project_root.to_owned(), Arc::clone(&candidate));
    Ok(candidate)
}

pub(crate) fn active_policy_snapshot(project_root: &str) -> Option<Arc<HookPolicySnapshot>> {
    snapshot_store()
        .read()
        .ok()
        .and_then(|snapshots| snapshots.get(project_root).cloned())
}

pub(crate) fn active_provider_projections(
    project_root: &str,
) -> Option<Vec<crate::protocol_activation::protocol_activation_manifest::HookProviderProjection>> {
    active_policy_snapshot(project_root).map(|snapshot| snapshot.provider_projections.clone())
}

#[cfg(test)]
#[path = "../tests/unit/hook_policy_kernel.rs"]
mod tests;

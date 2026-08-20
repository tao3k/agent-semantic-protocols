use agent_semantic_config::HookClientLanguageProviderConfig;
use agent_semantic_content_identity::exact_selector_merkle::canonical_content_digest_v1;
use std::collections::BTreeSet;

pub(crate) const HOOK_POLICY_KERNEL_VERSION: &str = "hook-enforcement-kernel-v1";

#[derive(Clone, Debug)]
pub(crate) struct HookPolicySnapshot {
    pub generation_digest: String,
    pub provider_projections:
        Vec<crate::protocol_activation::protocol_activation_manifest::HookProviderProjection>,
}

pub(crate) fn compile_language_provider_snapshot(
    language_providers: &[HookClientLanguageProviderConfig],
) -> Result<HookPolicySnapshot, String> {
    compile_language_provider_snapshot_inner(language_providers)
}

fn compile_language_provider_snapshot_inner(
    language_providers: &[HookClientLanguageProviderConfig],
) -> Result<HookPolicySnapshot, String> {
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
    let provider_projections = compile_provider_projections(&providers)?;
    Ok(HookPolicySnapshot {
        generation_digest: format!("blake3-256:{}", digest.as_str()),
        provider_projections,
    })
}

fn compile_provider_projections(
    providers: &[HookClientLanguageProviderConfig],
) -> Result<
    Vec<crate::protocol_activation::protocol_activation_manifest::HookProviderProjection>,
    String,
> {
    if providers.is_empty() {
        return Ok(Vec::new());
    }
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
            let routes =
                crate::materialize_provider_routes(manifest).map_err(|error| error.to_string())?;
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
                    owner_route: routes.owner,
                    lexical_route: routes.lexical,
                    ingest_route: routes.ingest,
                },
            )
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(provider_projections)
}

#[cfg(test)]
#[path = "../tests/unit/hook_policy_kernel.rs"]
mod tests;

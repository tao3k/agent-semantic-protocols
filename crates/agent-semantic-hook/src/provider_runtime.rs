use std::path::Path;

use crate::provider_manifest::{
    ProviderCommandSelectionScopeV1, activate_provider, activation_capability_coverage,
    provider_command_selections_for_scope_with_paths, provider_manifests,
};
use crate::{
    ActivationGeneratedBy, CANONICAL_SCHEMA_AUTHORITY, HOOK_ACTIVATION_SCHEMA_ID,
    HOOK_ACTIVATION_SCHEMA_VERSION, HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookActivation,
    HookRuntime,
};

/// Resolve only the selected language providers into an in-memory runtime.
///
/// Provider cold start deliberately excludes the ASP binary and Graph Turbo.
/// Those artifacts are owned by the resident Runtime Server and are not inputs
/// to a language provider selection.
pub(crate) fn build_provider_runtime_for_scope(
    project_root: &Path,
    scope: &ProviderCommandSelectionScopeV1,
) -> Result<HookRuntime, String> {
    let state_paths = agent_semantic_runtime::project_state_paths(project_root)
        .map_err(|error| format!("failed to resolve ASP project state paths: {error}"))?;
    let selections =
        provider_command_selections_for_scope_with_paths(project_root, &state_paths, scope)?;
    let manifests = provider_manifests();
    let semantic_registry_digest = crate::provider_registry::semantic_registry_digest();
    let providers = selections
        .iter()
        .map(|selection| {
            let manifest = manifests
                .iter()
                .find(|manifest| manifest.manifest_id == selection.manifest_id)
                .ok_or_else(|| {
                    format!(
                        "selected provider manifest is unavailable: manifestId={}",
                        selection.manifest_id
                    )
                })?;
            let coverage = activation_capability_coverage(manifest)?;
            activate_provider(
                manifest,
                selection.manifest_digest.clone(),
                selection.execution_command_digest.clone(),
                selection.binary.clone(),
                coverage,
                &semantic_registry_digest,
            )
        })
        .collect::<Result<Vec<_>, String>>()?;

    if providers.is_empty() {
        return Err("provider runtime selection produced no providers".to_string());
    }

    crate::activation_store::activation_to_runtime(&HookActivation {
        schema_id: HOOK_ACTIVATION_SCHEMA_ID.to_string(),
        schema_version: HOOK_ACTIVATION_SCHEMA_VERSION.to_string(),
        schema_authority: CANONICAL_SCHEMA_AUTHORITY.to_string(),
        protocol_id: HOOK_PROTOCOL_ID.to_string(),
        protocol_version: HOOK_PROTOCOL_VERSION.to_string(),
        project_root: project_root.display().to_string(),
        rankers: Vec::new(),
        providers,
        generated_by: ActivationGeneratedBy {
            runtime: "asp-runtime-server".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
        generated_at: None,
    })
}

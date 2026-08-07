//! Resolves current source-index snapshots from an activated provider runtime.

use std::path::Path;

use super::{
    CurrentSourceIndexSnapshot, LanguageId, ProviderId, ProviderRegistrySnapshot,
    SourceIndexOwnerPath, current_source_index_snapshot_for_owner_with_registry,
    current_source_index_snapshot_with_registry,
};
use crate::source_index::{
    ProviderSourceEnvelopeLookupRequestV1,
    ensure_provider_source_index_snapshot_at_artifact_root_with_registry,
    provider_source_snapshot_envelope_path_at_artifact_root_with_registry,
};
use agent_semantic_client_core::ProjectContext;

/// Inputs for capturing one exact owner from an already-loaded activation.
#[non_exhaustive]
pub struct CurrentSourceIndexOwnerFromActivationRequest<'a> {
    project_root: &'a Path,
    activation_path: &'a Path,
    activation: &'a agent_semantic_hook::HookRuntime,
    owner_path: SourceIndexOwnerPath,
    language_id: LanguageId,
    provider_id: ProviderId,
}

impl<'a>
    From<(
        &'a Path,
        &'a Path,
        &'a agent_semantic_hook::HookRuntime,
        SourceIndexOwnerPath,
        LanguageId,
        ProviderId,
    )> for CurrentSourceIndexOwnerFromActivationRequest<'a>
{
    fn from(
        value: (
            &'a Path,
            &'a Path,
            &'a agent_semantic_hook::HookRuntime,
            SourceIndexOwnerPath,
            LanguageId,
            ProviderId,
        ),
    ) -> Self {
        Self {
            project_root: value.0,
            activation_path: value.1,
            activation: value.2,
            owner_path: value.3,
            language_id: value.4,
            provider_id: value.5,
        }
    }
}

/// Capture the current content-authoritative source snapshot from an activation
/// that the command boundary already loaded.
pub async fn current_source_index_snapshot_from_activation(
    project_root: &Path,
    activation_path: &Path,
    activation: &agent_semantic_hook::HookRuntime,
) -> Result<CurrentSourceIndexSnapshot, String> {
    let provider_registry = ProviderRegistrySnapshot::from_activation(activation_path, activation)?;
    current_source_index_snapshot_with_registry(project_root, &provider_registry).await
}

/// Capture the complete source scope for one provider from an activation that
/// the command boundary already loaded.
pub async fn current_provider_source_index_snapshot_from_activation(
    project_root: &Path,
    activation_path: &Path,
    activation: &agent_semantic_hook::HookRuntime,
    language_id: &LanguageId,
    provider_id: &ProviderId,
) -> Result<CurrentSourceIndexSnapshot, String> {
    let provider_registry = ProviderRegistrySnapshot::from_activation(activation_path, activation)?;
    crate::source_index::current_live_provider_source_index_snapshot_with_registry(
        project_root,
        language_id,
        provider_id,
        &provider_registry,
    )
    .await
}

/// Refresh and publish one provider workspace envelope from an activation.
pub async fn ensure_provider_source_index_snapshot_from_activation(
    project_root: &Path,
    activation_path: &Path,
    activation: &agent_semantic_hook::HookRuntime,
    language_id: &LanguageId,
    provider_id: &ProviderId,
) -> Result<CurrentSourceIndexSnapshot, String> {
    let provider_registry = ProviderRegistrySnapshot::from_activation(activation_path, activation)?;
    let project_context = ProjectContext::resolve(project_root)?;
    ensure_provider_source_index_snapshot_at_artifact_root_with_registry(
        ProviderSourceEnvelopeLookupRequestV1 {
            project_root,
            artifact_root: project_context.state_layout().artifacts_dir(),
            language_id,
            provider_id,
            provider_registry: &provider_registry,
        },
    )
    .await
}

/// Resolve the canonical pre-published envelope path from an activation.
pub fn provider_source_snapshot_envelope_path_from_activation(
    project_root: &Path,
    activation_path: &Path,
    activation: &agent_semantic_hook::HookRuntime,
    language_id: &LanguageId,
    provider_id: &ProviderId,
) -> Result<std::path::PathBuf, String> {
    let provider_registry = ProviderRegistrySnapshot::from_activation(activation_path, activation)?;
    let project_context = ProjectContext::resolve(project_root)?;
    provider_source_snapshot_envelope_path_at_artifact_root_with_registry(
        ProviderSourceEnvelopeLookupRequestV1 {
            project_root,
            artifact_root: project_context.state_layout().artifacts_dir(),
            language_id,
            provider_id,
            provider_registry: &provider_registry,
        },
    )
}

/// Capture one exact owner from an activation that the caller already loaded.
///
/// This keeps activation synchronization at the command boundary instead of
/// re-entering the manifest/activation materializer from the source snapshot.
pub fn current_source_index_snapshot_for_owner_from_activation<'a>(
    request: impl Into<CurrentSourceIndexOwnerFromActivationRequest<'a>>,
) -> Result<CurrentSourceIndexSnapshot, String> {
    let request = request.into();
    let provider_registry =
        ProviderRegistrySnapshot::from_activation(request.activation_path, request.activation)?;
    current_source_index_snapshot_for_owner_with_registry(
        request.project_root,
        request.owner_path.as_str(),
        request.language_id.as_str(),
        request.provider_id.as_str(),
        &provider_registry,
    )
}

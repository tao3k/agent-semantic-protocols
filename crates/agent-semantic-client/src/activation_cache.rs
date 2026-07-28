//! Turso-backed provider activation selection guard.

use std::path::Path;

use agent_semantic_client_core::ProviderRegistrySnapshot;

pub(crate) fn load_provider_registry_snapshot(
    activation_root: &Path,
    _project_root: &Path,
    _emit_stderr_diagnostics: bool,
) -> Result<ProviderRegistrySnapshot, String> {
    ProviderRegistrySnapshot::load(activation_root)
}

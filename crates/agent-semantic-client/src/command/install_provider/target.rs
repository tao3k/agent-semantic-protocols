//! Provider install target resolution for language harness binaries.

use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct ProviderBinaryInstallTarget {
    pub(super) path: PathBuf,
    pub(super) source: &'static str,
}

/// Resolve the canonical binary target for one provider installation.
pub(super) fn resolve_provider_binary_install_target(
    language_id: &str,
    provider_binary: &str,
) -> Result<ProviderBinaryInstallTarget, String> {
    let state_home =
        agent_semantic_runtime::state_core::resolve_state_home().map_err(|error| {
            format!(
                "failed to resolve ASP State Home for provider `{provider_binary}` language `{language_id}`: {error}"
            )
        })?;
    resolve_provider_binary_install_target_at(language_id, provider_binary, &state_home)
}

fn resolve_provider_binary_install_target_at(
    language_id: &str,
    provider_binary: &str,
    state_home: &Path,
) -> Result<ProviderBinaryInstallTarget, String> {
    Ok(ProviderBinaryInstallTarget {
        path: state_home_provider_binary_at(state_home, provider_binary, language_id)?,
        source: "state-home-runtime-bin",
    })
}

fn state_home_provider_binary_at(
    state_home: &Path,
    binary: &str,
    language_id: &str,
) -> Result<PathBuf, String> {
    let binary_path = Path::new(binary);
    if binary_path.components().count() != 1
        || binary_path.file_name().and_then(|name| name.to_str()) != Some(binary)
    {
        return Err(format!(
            "provider binary for language `{language_id}` must be a logical basename resolved under State Home runtime/bin, got `{binary}`"
        ));
    }
    Ok(state_home.join("runtime").join("bin").join(binary_path))
}

#[cfg(test)]
#[path = "../../../tests/unit/install_provider_target.rs"]
mod install_provider_target_tests;

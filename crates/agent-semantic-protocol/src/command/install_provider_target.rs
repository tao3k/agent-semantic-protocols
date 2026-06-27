//! Provider install target resolution for language harness binaries.

use std::env;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct ProviderBinaryInstallTarget {
    pub(super) path: PathBuf,
    pub(super) source: &'static str,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct ProviderBinaryInvocation {
    pub(super) command: String,
    pub(super) source: &'static str,
}

pub(super) fn resolve_provider_binary_install_target(
    language_id: &str,
    provider_binary: &str,
    home_dir: Option<&Path>,
) -> Result<ProviderBinaryInstallTarget, String> {
    let bin_dir = semantic_agent_bin_dir();
    resolve_provider_binary_install_target_with_bin_dir(
        language_id,
        provider_binary,
        home_dir,
        bin_dir.as_deref(),
    )
}

fn resolve_provider_binary_install_target_with_bin_dir(
    language_id: &str,
    provider_binary: &str,
    home_dir: Option<&Path>,
    semantic_agent_bin_dir: Option<&Path>,
) -> Result<ProviderBinaryInstallTarget, String> {
    if let Some(bin_dir) = semantic_agent_bin_dir {
        return Ok(ProviderBinaryInstallTarget {
            path: bin_dir.join(provider_binary),
            source: "semantic-agent-bin-dir",
        });
    }
    Ok(ProviderBinaryInstallTarget {
        path: home_local_bin_required(provider_binary, home_dir, language_id)?,
        source: "home-local-bin",
    })
}

pub(super) fn resolve_provider_binary_invocation(
    language_id: &str,
    provider_binary: &str,
    home_dir: Option<&Path>,
) -> Result<ProviderBinaryInvocation, String> {
    let home_bin = home_local_bin_required(provider_binary, home_dir, language_id)?;
    if !home_bin.is_file() {
        return Err(format!(
            "provider binary `{provider_binary}` for language `{language_id}` must be installed at {}; run `asp install language {language_id}`",
            home_bin.display()
        ));
    }
    Ok(ProviderBinaryInvocation {
        command: home_bin.to_string_lossy().to_string(),
        source: "home-local-bin",
    })
}

fn home_local_bin(binary: &str, home_dir: Option<&Path>) -> Option<PathBuf> {
    home_dir.map(|home_dir| home_dir.join(".local/bin").join(binary))
}

fn home_local_bin_required(
    binary: &str,
    home_dir: Option<&Path>,
    language_id: &str,
) -> Result<PathBuf, String> {
    home_local_bin(binary, home_dir).ok_or_else(|| {
        format!(
            "provider binary `{binary}` for language `{language_id}` must be installed at $HOME/.local/bin/{binary}; HOME is not set"
        )
    })
}

pub(super) fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn semantic_agent_bin_dir() -> Option<PathBuf> {
    env::var_os("SEMANTIC_AGENT_BIN_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
#[path = "../../tests/unit/install_provider_target.rs"]
mod install_provider_target_tests;

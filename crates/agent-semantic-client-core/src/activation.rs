//! Provider activation snapshot loading for `agent-semantic-client`.

use std::fs;
use std::path::{Path, PathBuf};

use agent_semantic_hook::{
    ActivatedProvider, builtin_provider_manifests, parse_activation, project_hook_state_dir,
};

use crate::receipt::NativeProvenance;
use crate::types::{LanguageId, ProviderId};

/// Provider resolved from the project hook activation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedProvider {
    pub language_id: LanguageId,
    pub provider_id: ProviderId,
    pub binary: String,
    pub provider_command_prefix: Vec<String>,
    pub package_roots: Vec<String>,
}

impl ResolvedProvider {
    /// Convert provider activation data into receipt provenance.
    #[must_use]
    pub fn provenance(&self) -> NativeProvenance {
        NativeProvenance {
            language_id: self.language_id.clone(),
            provider_id: self.provider_id.clone(),
            provider_binary: self.binary.clone(),
        }
    }

    /// Return the registry-owned provider command prefix.
    #[must_use]
    pub fn command_prefix(&self) -> Vec<String> {
        if self.provider_command_prefix.is_empty() {
            vec![self.binary.clone()]
        } else {
            self.provider_command_prefix.clone()
        }
    }
}

impl From<&ActivatedProvider> for ResolvedProvider {
    fn from(provider: &ActivatedProvider) -> Self {
        Self {
            language_id: provider.language_id.clone().into(),
            provider_id: provider.provider_id.clone().into(),
            binary: provider.binary.clone(),
            provider_command_prefix: provider.provider_command_prefix.clone(),
            package_roots: provider.package_roots.clone(),
        }
    }
}

/// Snapshot of activated providers for one project root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderRegistrySnapshot {
    pub activation_path: PathBuf,
    pub providers: Vec<ResolvedProvider>,
}

impl ProviderRegistrySnapshot {
    /// Load provider activation from the default project hook cache path.
    pub fn load(project_root: &Path) -> Result<Self, String> {
        let activation_path = project_hook_state_dir(project_root)
            .map_err(|error| error.to_string())?
            .join("activation.json");
        Self::load_from_path(&activation_path)
    }

    /// Load provider activation from an explicit `activation.json` path.
    pub fn load_from_path(activation_path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(activation_path).map_err(|error| {
            format!(
                "failed to read agent semantic client provider activation at {}: {error}",
                activation_path.display()
            )
        })?;
        let manifests = builtin_provider_manifests();
        let activation =
            parse_activation(&text, &manifests).map_err(|error| format!("{error:?}"))?;
        Ok(Self {
            activation_path: activation_path.to_path_buf(),
            providers: activation
                .providers
                .iter()
                .map(ResolvedProvider::from)
                .collect(),
        })
    }

    /// Resolve the activated provider for a language id.
    #[must_use]
    pub fn provider_for_language(&self, language_id: &LanguageId) -> Option<&ResolvedProvider> {
        self.providers
            .iter()
            .find(|provider| &provider.language_id == language_id)
    }

    /// Return provenance for every activated provider.
    #[must_use]
    pub fn native_provenance(&self) -> Vec<NativeProvenance> {
        self.providers
            .iter()
            .map(ResolvedProvider::provenance)
            .collect()
    }
}

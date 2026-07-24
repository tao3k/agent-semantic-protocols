//! Provider-manifest-owned source and config file boundaries for search.

use agent_semantic_hook::builtin_provider_manifests;
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchLanguageId(String);

impl SearchLanguageId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for SearchLanguageId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for SearchLanguageId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

/// Source/config file matcher built from provider manifest defaults.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LanguageFileSpec {
    extensions: Vec<String>,
    config_filenames: Vec<String>,
    project_markers: Vec<String>,
    dependency_markers: Vec<String>,
}

impl LanguageFileSpec {
    /// Source extensions covered by this language file spec.
    #[must_use]
    pub fn extensions(&self) -> &[String] {
        self.extensions.as_slice()
    }

    /// Config filenames covered by this language file spec.
    #[must_use]
    pub fn config_filenames(&self) -> &[String] {
        self.config_filenames.as_slice()
    }

    /// Project marker filenames covered by this language file spec.
    #[must_use]
    pub fn project_markers(&self) -> &[String] {
        self.project_markers.as_slice()
    }

    /// Dependency marker filenames covered by this language file spec.
    #[must_use]
    pub fn dependency_markers(&self) -> &[String] {
        self.dependency_markers.as_slice()
    }

    /// Return true when `path` is a provider source/config path.
    #[must_use]
    pub fn matches(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                self.extensions
                    .iter()
                    .any(|source_extension| source_extension.trim_start_matches('.') == extension)
            })
            || self.is_config_path(path)
    }

    /// Return true when `path` matches a provider config filename.
    #[must_use]
    pub fn is_config_path(&self, path: &Path) -> bool {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                self.config_filenames
                    .iter()
                    .any(|config_filename| config_filename == name)
            })
    }

    fn from_provider_defaults<'a>(
        extensions: impl Iterator<Item = &'a String>,
        config_filenames: impl Iterator<Item = &'a String>,
        project_markers: impl Iterator<Item = &'a String>,
        dependency_markers: impl Iterator<Item = &'a String>,
    ) -> Self {
        Self {
            extensions: unique_strings(extensions),
            config_filenames: unique_strings(config_filenames),
            project_markers: unique_strings(project_markers),
            dependency_markers: unique_strings(dependency_markers),
        }
    }
}

/// Build a language-specific file matcher from provider manifest defaults.
#[must_use]
pub fn language_file_spec(language_id: &SearchLanguageId) -> LanguageFileSpec {
    let manifests = builtin_provider_manifests();
    LanguageFileSpec::from_provider_defaults(
        manifests
            .iter()
            .filter(|manifest| manifest.language_id == language_id.as_str())
            .flat_map(|manifest| manifest.source.default_extensions.iter()),
        manifests
            .iter()
            .filter(|manifest| manifest.language_id == language_id)
            .flat_map(|manifest| manifest.source.default_config_files.iter()),
        manifests
            .iter()
            .filter(|manifest| manifest.language_id == language_id)
            .flat_map(|manifest| manifest.source.default_project_markers.iter()),
        manifests
            .iter()
            .filter(|manifest| manifest.language_id == language_id)
            .flat_map(|manifest| manifest.source.default_dependency_markers.iter()),
    )
}

/// Build a language-neutral matcher from all provider manifest defaults.
#[must_use]
pub fn language_neutral_search_file_spec() -> LanguageFileSpec {
    let manifests = builtin_provider_manifests();
    LanguageFileSpec::from_provider_defaults(
        manifests
            .iter()
            .flat_map(|manifest| manifest.source.default_extensions.iter()),
        manifests
            .iter()
            .flat_map(|manifest| manifest.source.default_config_files.iter()),
        manifests
            .iter()
            .flat_map(|manifest| manifest.source.default_project_markers.iter()),
        manifests
            .iter()
            .flat_map(|manifest| manifest.source.default_dependency_markers.iter()),
    )
}

fn unique_strings<'a>(values: impl Iterator<Item = &'a String>) -> Vec<String> {
    values
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

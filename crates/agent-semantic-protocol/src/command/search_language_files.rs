//! Language-owned source and config file boundaries for ASP search surfaces.

use agent_semantic_hook::builtin_provider_manifests;
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct LanguageFileSpec {
    extensions: Vec<String>,
    config_filenames: Vec<String>,
}

impl LanguageFileSpec {
    pub(super) fn extensions(&self) -> &[String] {
        self.extensions.as_slice()
    }

    pub(super) fn config_filenames(&self) -> &[String] {
        self.config_filenames.as_slice()
    }

    pub(super) fn matches(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                self.extensions
                    .iter()
                    .any(|source_extension| source_extension.trim_start_matches('.') == extension)
            })
            || self.is_config_path(path)
    }

    pub(super) fn is_config_path(&self, path: &Path) -> bool {
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
    ) -> Self {
        Self {
            extensions: unique_strings(extensions),
            config_filenames: unique_strings(config_filenames),
        }
    }
}

pub(super) fn language_file_spec(language_id: &str) -> LanguageFileSpec {
    let manifests = builtin_provider_manifests();
    LanguageFileSpec::from_provider_defaults(
        manifests
            .iter()
            .filter(|manifest| manifest.language_id == language_id)
            .flat_map(|manifest| manifest.source.default_extensions.iter()),
        manifests
            .iter()
            .filter(|manifest| manifest.language_id == language_id)
            .flat_map(|manifest| manifest.source.default_config_files.iter()),
    )
}

pub(super) fn language_neutral_search_file_spec() -> LanguageFileSpec {
    let manifests = builtin_provider_manifests();
    LanguageFileSpec::from_provider_defaults(
        manifests
            .iter()
            .flat_map(|manifest| manifest.source.default_extensions.iter()),
        manifests
            .iter()
            .flat_map(|manifest| manifest.source.default_config_files.iter()),
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

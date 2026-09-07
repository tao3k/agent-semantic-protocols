// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Runtime-injected source and config file boundaries for search.

use std::path::Path;

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

    /// Construct a matcher from a Runtime Server admitted provider scope.
    #[must_use]
    pub fn from_runtime_scope(
        extensions: Vec<String>,
        config_filenames: Vec<String>,
        project_markers: Vec<String>,
        dependency_markers: Vec<String>,
    ) -> Self {
        Self {
            extensions,
            config_filenames,
            project_markers,
            dependency_markers,
        }
    }
}

/// Return a fail-closed matcher when no Runtime Server provider scope was supplied.
///
/// Language IDs are identities, not static filename registries. Callers that
/// own an admitted provider generation must pass `from_runtime_scope` through
/// `file_spec_override`; search never reconstructs provider scope locally.
#[must_use]
pub fn language_file_spec(_language_id: impl AsRef<str>) -> LanguageFileSpec {
    LanguageFileSpec::default()
}

/// Return a fail-closed language-neutral matcher without a Runtime scope.
#[must_use]
pub fn language_neutral_search_file_spec() -> LanguageFileSpec {
    LanguageFileSpec::default()
}

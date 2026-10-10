// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Runtime provider status rendering and source selector matching.

use globset::GlobBuilder;
use globset::GlobSet;
use globset::GlobSetBuilder;

use crate::protocol::normalize_source_selector;

use crate::provider_projection::HookProviderProjection;
use crate::provider_projection::HookRuntime;
use crate::provider_projection::ProviderSelectorMatch;
use crate::provider_projection::SourceSelectorKind;

impl HookRuntime {
    pub(crate) fn providers_for_selector(&self, selector: &str) -> Vec<ProviderSelectorMatch> {
        if !selector_is_workspace_owned(self, selector) {
            return Vec::new();
        }
        let matcher = SourceSelectorMatcher::new(selector);
        self.policy_providers
            .iter()
            .filter_map(|provider| {
                provider
                    .match_source_selector_with(&matcher)
                    .map(|kind| ProviderSelectorMatch {
                        provider: provider.clone(),
                        kind,
                    })
            })
            .collect()
    }
}

fn selector_is_workspace_owned(runtime: &HookRuntime, selector: &str) -> bool {
    let selector = normalize_source_selector(selector);
    if selector.contains("://") {
        return true;
    }
    let Ok(current_dir) = std::env::current_dir() else {
        return false;
    };
    let root = lexical_absolute_path(&current_dir, std::path::Path::new(&runtime.project_root));
    let candidate = lexical_absolute_path(&root, std::path::Path::new(selector));
    candidate.starts_with(&root)
}

fn lexical_absolute_path(base: &std::path::Path, path: &std::path::Path) -> std::path::PathBuf {
    use std::path::Component;

    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    };
    let mut normalized = std::path::PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

impl HookProviderProjection {
    fn match_source_selector_with(
        &self,
        selector: &SourceSelectorMatcher<'_>,
    ) -> Option<SourceSelectorKind> {
        if selector.has_glob
            && self
                .source_extensions
                .iter()
                .any(|extension| selector.targets_extension(extension))
        {
            return Some(SourceSelectorKind::Pattern);
        }
        if !selector.has_glob
            && self
                .source_extensions
                .iter()
                .any(|extension| selector.normalized.ends_with(extension))
        {
            return Some(SourceSelectorKind::ExactPath);
        }
        let path_shaped = selector.normalized == "." || selector.normalized.contains('/');
        let selector_has_extension = std::path::Path::new(selector.normalized)
            .extension()
            .is_some();
        if path_shaped
            && self.package_roots.iter().any(|root| {
                let exact_root = selector.normalized == root;
                let root_scope = root == "."
                    || exact_root
                    || selector
                        .normalized
                        .strip_prefix(root)
                        .is_some_and(|suffix| suffix.starts_with('/'))
                    || selector
                        .normalized
                        .match_indices(root)
                        .any(|(start, matched)| {
                            let end = start + matched.len();
                            (start == 0
                                || selector.normalized.as_bytes().get(start.wrapping_sub(1))
                                    == Some(&b'/'))
                                && (end == selector.normalized.len()
                                    || selector.normalized.as_bytes().get(end) == Some(&b'/'))
                        });
                root_scope && (exact_root || !selector_has_extension)
            })
        {
            return Some(SourceSelectorKind::Pattern);
        }
        self.config_files
            .iter()
            .any(|config| selector.normalized.ends_with(config))
            .then_some(SourceSelectorKind::ExactPath)
    }
}

struct SourceSelectorMatcher<'a> {
    normalized: &'a str,
    has_glob: bool,
    extension_glob: Option<GlobSet>,
}

impl<'a> SourceSelectorMatcher<'a> {
    fn new(selector: &'a str) -> Self {
        let normalized = normalize_source_selector(selector);
        let has_glob = selector_has_glob(normalized);
        let basename = basename_pattern(normalized).to_ascii_lowercase();
        let extension_pattern = basename_extension_pattern(&basename);
        Self {
            normalized,
            has_glob,
            extension_glob: extension_pattern.and_then(build_glob_set),
        }
    }

    fn targets_extension(&self, extension: &str) -> bool {
        let extension = extension.trim_start_matches('.').to_ascii_lowercase();
        self.extension_glob
            .as_ref()
            .is_some_and(|glob_set| glob_set.is_match(extension))
    }
}

fn build_glob_set(pattern: &str) -> Option<GlobSet> {
    let glob = GlobBuilder::new(pattern)
        .literal_separator(false)
        .backslash_escape(false)
        .build()
        .ok()?;
    let mut builder = GlobSetBuilder::new();
    builder.add(glob);
    builder.build().ok()
}

fn basename_pattern(selector: &str) -> &str {
    selector
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(selector)
}

fn basename_extension_pattern(basename: &str) -> Option<&str> {
    let (_, _, last_literal_dot) = basename.char_indices().fold(
        (0usize, 0usize, None),
        |(bracket_depth, brace_depth, last_literal_dot), (index, character)| match character {
            '[' => (bracket_depth + 1, brace_depth, last_literal_dot),
            ']' if bracket_depth > 0 => (bracket_depth - 1, brace_depth, last_literal_dot),
            '{' if bracket_depth == 0 => (bracket_depth, brace_depth + 1, last_literal_dot),
            '}' if bracket_depth == 0 && brace_depth > 0 => {
                (bracket_depth, brace_depth - 1, last_literal_dot)
            }
            '.' if bracket_depth == 0 && brace_depth == 0 => {
                (bracket_depth, brace_depth, Some(index))
            }
            _ => (bracket_depth, brace_depth, last_literal_dot),
        },
    );
    let start = last_literal_dot? + 1;
    (start < basename.len()).then_some(&basename[start..])
}

fn selector_has_glob(path: &str) -> bool {
    path.chars()
        .any(|character| matches!(character, '*' | '?' | '[' | ']' | '{' | '}'))
}

#[cfg(test)]
#[path = "../tests/unit/provider_routing.rs"]
mod tests;

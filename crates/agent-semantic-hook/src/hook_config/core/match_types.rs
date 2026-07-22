use std::collections::HashSet;

use aho_corasick::AhoCorasick;
use globset::GlobSet;

#[derive(Debug, Default)]
pub(super) struct CompiledCommandContains {
    pub(super) matcher: Option<AhoCorasick>,
}

impl CompiledCommandContains {
    pub(super) fn is_empty(&self) -> bool {
        self.matcher.is_none()
    }

    pub(super) fn matches(&self, command: &str) -> bool {
        self.matcher
            .as_ref()
            .is_some_and(|matcher| matcher.is_match(command))
    }
}

#[derive(Debug, Default)]
pub(super) struct CompiledPathGlobs {
    pub(super) suffix_ext_any: HashSet<String>,
    pub(super) suffix_any: Vec<String>,
    pub(super) globset: Option<GlobSet>,
}

impl CompiledPathGlobs {
    pub(super) fn is_empty(&self) -> bool {
        self.suffix_ext_any.is_empty() && self.suffix_any.is_empty() && self.globset.is_none()
    }

    pub(super) fn is_suffix_only(&self) -> bool {
        (!self.suffix_ext_any.is_empty() || !self.suffix_any.is_empty()) && self.globset.is_none()
    }

    pub(super) fn matches(&self, path: &str) -> bool {
        self.matches_suffix_extension(path)
            || self.suffix_any.iter().any(|suffix| path.ends_with(suffix))
            || self
                .globset
                .as_ref()
                .is_some_and(|globset| globset.is_match(path))
    }

    pub(super) fn matches_suffix_extension(&self, path: &str) -> bool {
        let Some(dot_index) = path.rfind('.') else {
            return false;
        };
        self.suffix_ext_any.contains(&path[dot_index..])
    }
}

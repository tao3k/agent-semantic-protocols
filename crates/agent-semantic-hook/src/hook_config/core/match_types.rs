use std::collections::HashSet;

use aho_corasick::AhoCorasick;
use aho_corasick::AhoCorasickBuilder;
use aho_corasick::MatchKind;
use globset::GlobBuilder;
use globset::GlobSet;
use globset::GlobSetBuilder;

/// Durable matcher facts remain declarative strings.  Globset and Aho are
/// rebuilt once at generation hydration; no regex bytecode is serialized.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct DurableCommandContainsMatcher {
    pub(super) patterns: Vec<String>,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct DurablePathGlobMatcher {
    pub(super) suffix_ext_any: Vec<String>,
    pub(super) suffix_any: Vec<String>,
    pub(super) patterns: Vec<String>,
}

#[derive(Debug, Default)]
pub(super) struct CompiledCommandContains {
    matcher: Option<AhoCorasick>,
    durable: DurableCommandContainsMatcher,
}

impl CompiledCommandContains {
    pub(super) fn live(matcher: AhoCorasick, durable: DurableCommandContainsMatcher) -> Self {
        Self {
            matcher: Some(matcher),
            durable,
        }
    }

    pub(super) fn from_durable(durable: DurableCommandContainsMatcher) -> Result<Self, String> {
        Ok(Self {
            matcher: build_command_contains(&durable.patterns)?,
            durable,
        })
    }

    pub(super) fn durable_artifact(&self) -> DurableCommandContainsMatcher {
        self.durable.clone()
    }
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
    matcher: Option<GlobSet>,
    durable: DurablePathGlobMatcher,
}

impl CompiledPathGlobs {
    pub(super) fn live(
        suffix_ext_any: HashSet<String>,
        suffix_any: Vec<String>,
        patterns: Vec<String>,
        matcher: Option<GlobSet>,
    ) -> Self {
        let mut durable_suffix_ext_any = suffix_ext_any.iter().cloned().collect::<Vec<_>>();
        durable_suffix_ext_any.sort();
        Self {
            suffix_ext_any,
            suffix_any: suffix_any.clone(),
            matcher,
            durable: DurablePathGlobMatcher {
                suffix_ext_any: durable_suffix_ext_any,
                suffix_any,
                patterns,
            },
        }
    }

    pub(super) fn from_durable(durable: DurablePathGlobMatcher) -> Result<Self, String> {
        Ok(Self {
            suffix_ext_any: durable.suffix_ext_any.iter().cloned().collect(),
            suffix_any: durable.suffix_any.clone(),
            matcher: build_globset("durable path glob", &durable.patterns)?,
            durable,
        })
    }

    pub(super) fn durable_artifact(&self) -> DurablePathGlobMatcher {
        self.durable.clone()
    }
    pub(super) fn is_empty(&self) -> bool {
        self.suffix_ext_any.is_empty() && self.suffix_any.is_empty() && self.matcher.is_none()
    }
    pub(super) fn is_suffix_only(&self) -> bool {
        (!self.suffix_ext_any.is_empty() || !self.suffix_any.is_empty()) && self.matcher.is_none()
    }
    pub(super) fn matches(&self, path: &str) -> bool {
        self.matches_suffix_extension(path)
            || self.suffix_any.iter().any(|suffix| path.ends_with(suffix))
            || self
                .matcher
                .as_ref()
                .is_some_and(|matcher| matcher.is_match(path))
    }
    pub(super) fn matches_suffix_extension(&self, path: &str) -> bool {
        path.rfind('.')
            .is_some_and(|index| self.suffix_ext_any.contains(&path[index..]))
    }
}

pub(super) fn build_command_contains(patterns: &[String]) -> Result<Option<AhoCorasick>, String> {
    if patterns.is_empty() {
        return Ok(None);
    }
    AhoCorasickBuilder::new()
        .ascii_case_insensitive(true)
        .match_kind(MatchKind::LeftmostFirst)
        .build(patterns)
        .map(Some)
        .map_err(|error| format!("failed to compile commandContainsAny patterns: {error}"))
}

pub(super) fn build_globset(label: &str, patterns: &[String]) -> Result<Option<GlobSet>, String> {
    if patterns.is_empty() {
        return Ok(None);
    }
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(
            GlobBuilder::new(pattern)
                .literal_separator(true)
                .build()
                .map_err(|error| format!("invalid {label} pattern `{pattern}`: {error}"))?,
        );
    }
    builder
        .build()
        .map(Some)
        .map_err(|error| format!("failed to compile {label} patterns: {error}"))
}

#[cfg(test)]
#[path = "../../../tests/unit/hook_config_match_types.rs"]
mod tests;

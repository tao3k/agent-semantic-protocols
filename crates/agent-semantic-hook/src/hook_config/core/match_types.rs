use std::collections::HashSet;

use aho_corasick::AhoCorasick;
use globset::GlobSet;
use regex_automata::{
    Input,
    dfa::{Automaton, dense::DFA},
};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct DurableDfaMatcher {
    #[serde(with = "base64_bytes")]
    bytes: Vec<u8>,
    padding: usize,
}

#[path = "match_types_base64_bytes.rs"]
mod base64_bytes;

impl DurableDfaMatcher {
    pub(super) fn new(bytes: Vec<u8>, padding: usize) -> Result<Self, String> {
        let matcher = Self { bytes, padding };
        matcher.validate()?;
        Ok(matcher)
    }

    fn dfa(&self) -> Result<DFA<&[u32]>, String> {
        let bytes = self
            .bytes
            .get(self.padding..)
            .ok_or_else(|| "durable matcher DFA padding exceeds artifact bytes".to_owned())?;
        DFA::from_bytes(bytes)
            .map(|(dfa, _)| dfa)
            .map_err(|error| format!("invalid durable matcher DFA artifact: {error}"))
    }

    pub(super) fn validate(&self) -> Result<(), String> {
        self.dfa().map(|_| ())
    }

    fn matches(&self, value: &str) -> bool {
        self.dfa()
            .ok()
            .and_then(|dfa| dfa.try_search_fwd(&Input::new(value)).ok())
            .flatten()
            .is_some()
    }
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct DurableCommandContainsMatcher {
    pub(super) dfa: Option<DurableDfaMatcher>,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct DurablePathGlobMatcher {
    pub(super) suffix_ext_any: Vec<String>,
    pub(super) suffix_any: Vec<String>,
    pub(super) dfa: Option<DurableDfaMatcher>,
}

#[derive(Debug)]
enum CommandContainsMatcher {
    Live(AhoCorasick),
    Durable(DurableDfaMatcher),
}

#[derive(Debug, Default)]
pub(super) struct CompiledCommandContains {
    matcher: Option<CommandContainsMatcher>,
    durable: DurableCommandContainsMatcher,
}

impl CompiledCommandContains {
    pub(super) fn live(matcher: AhoCorasick, durable: DurableCommandContainsMatcher) -> Self {
        Self {
            matcher: Some(CommandContainsMatcher::Live(matcher)),
            durable,
        }
    }

    pub(super) fn from_durable(durable: DurableCommandContainsMatcher) -> Result<Self, String> {
        Ok(Self {
            matcher: durable.dfa.clone().map(CommandContainsMatcher::Durable),
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
        self.matcher.as_ref().is_some_and(|matcher| match matcher {
            CommandContainsMatcher::Live(matcher) => matcher.is_match(command),
            CommandContainsMatcher::Durable(matcher) => matcher.matches(command),
        })
    }
}

#[derive(Debug)]
enum PathGlobMatcher {
    Live(GlobSet),
    Durable(DurableDfaMatcher),
}

#[derive(Debug, Default)]
pub(super) struct CompiledPathGlobs {
    pub(super) suffix_ext_any: HashSet<String>,
    pub(super) suffix_any: Vec<String>,
    matcher: Option<PathGlobMatcher>,
    durable: DurablePathGlobMatcher,
}

impl CompiledPathGlobs {
    pub(super) fn live(
        suffix_ext_any: HashSet<String>,
        suffix_any: Vec<String>,
        globset: Option<GlobSet>,
        dfa: Option<DurableDfaMatcher>,
    ) -> Self {
        let mut durable_suffix_ext_any = suffix_ext_any.iter().cloned().collect::<Vec<_>>();
        durable_suffix_ext_any.sort();
        Self {
            suffix_ext_any,
            suffix_any: suffix_any.clone(),
            matcher: globset.map(PathGlobMatcher::Live),
            durable: DurablePathGlobMatcher {
                suffix_ext_any: durable_suffix_ext_any,
                suffix_any,
                dfa,
            },
        }
    }

    pub(super) fn from_durable(durable: DurablePathGlobMatcher) -> Result<Self, String> {
        Ok(Self {
            suffix_ext_any: durable.suffix_ext_any.iter().cloned().collect(),
            suffix_any: durable.suffix_any.clone(),
            matcher: durable.dfa.clone().map(PathGlobMatcher::Durable),
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
            || self.matcher.as_ref().is_some_and(|matcher| match matcher {
                PathGlobMatcher::Live(globset) => globset.is_match(path),
                PathGlobMatcher::Durable(dfa) => dfa.matches(path),
            })
    }

    pub(super) fn matches_suffix_extension(&self, path: &str) -> bool {
        let Some(dot_index) = path.rfind('.') else {
            return false;
        };
        self.suffix_ext_any.contains(&path[dot_index..])
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/hook_config_match_types.rs"]
mod tests;

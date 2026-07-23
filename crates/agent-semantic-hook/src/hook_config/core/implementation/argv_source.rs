//! Matches source-bearing argv tokens after wrapper normalization.

use std::collections::BTreeSet;

use super::compiled_rule::RuleMatch;
use crate::{HookRuntime, source_selector::collect_source_selector_matches};

impl RuleMatch {
    pub(in crate::hook_config) fn matching_argv_source_paths(
        &self,
        runtime: &HookRuntime,
        command_tokens: Option<&[String]>,
    ) -> Option<Vec<String>> {
        let project_root = std::path::Path::new(&runtime.project_root);
        if self.argv_source_any.is_empty()
            && self.argv_source_glob_any.is_empty()
            && !self.argv_workspace_regular_file
            && !self.argv_registered_source_file
        {
            return Some(Vec::new());
        }
        let tokens = command_tokens?;

        let mut paths = BTreeSet::new();
        let mut skip_next = false;
        let mut positional_only = false;
        for token in tokens {
            if skip_next {
                skip_next = false;
                continue;
            }
            if token == "--" {
                positional_only = true;
                continue;
            }
            if !positional_only
                && self
                    .argv_source_exclude_flag_any
                    .iter()
                    .any(|flag| token.as_str() == flag.as_str())
            {
                skip_next = true;
                continue;
            }
            if !positional_only
                && self.argv_source_exclude_flag_any.iter().any(|flag| {
                    token
                        .strip_prefix(flag.as_str())
                        .is_some_and(|suffix| suffix.starts_with('='))
                })
            {
                continue;
            }
            if self.argv_registered_source_file {
                let provider_owned =
                    !collect_source_selector_matches(runtime, [token.as_str()], |_| true)
                        .is_empty();
                if provider_owned {
                    paths.insert(token.to_string());
                }
            } else if self.argv_source_glob_any.is_suffix_only() {
                if let Some(path) = self.fast_argv_source_path(project_root, token) {
                    paths.insert(path);
                }
            } else {
                if self.matches_argv_source_path(project_root, token) {
                    paths.insert(token.to_string());
                }
            }
        }
        if self.argv_registered_source_file {
            for candidate in agent_semantic_command_match::embedded_literal_candidates(tokens) {
                let provider_owned =
                    !collect_source_selector_matches(runtime, [candidate.as_str()], |_| true)
                        .is_empty();
                if provider_owned {
                    paths.insert(candidate);
                }
            }
        }
        (!paths.is_empty()).then(|| paths.into_iter().collect())
    }
}

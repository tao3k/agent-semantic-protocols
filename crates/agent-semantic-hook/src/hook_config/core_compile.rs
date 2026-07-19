use std::collections::HashSet;

use aho_corasick::{AhoCorasickBuilder, MatchKind};
use globset::{GlobBuilder, GlobSetBuilder};

use crate::hook_config::core_match_types::{CompiledCommandContains, CompiledPathGlobs};

pub(super) fn compile_globs(
    label: &str,
    patterns: Vec<String>,
) -> Result<CompiledPathGlobs, String> {
    if patterns.is_empty() {
        return Ok(CompiledPathGlobs::default());
    }
    let paired_suffixes = paired_suffix_globs(&patterns);
    let mut suffix_ext_any = HashSet::new();
    let mut suffix_any = Vec::new();
    for suffix in &paired_suffixes {
        if simple_extension_suffix(suffix) {
            suffix_ext_any.insert(suffix.clone());
        } else {
            suffix_any.push(suffix.clone());
        }
    }
    let mut builder = GlobSetBuilder::new();
    let mut glob_count = 0usize;
    for pattern in patterns {
        if simple_glob_suffix(&pattern)
            .is_some_and(|suffix| paired_suffixes.iter().any(|existing| existing == suffix))
        {
            continue;
        }
        let glob = GlobBuilder::new(&pattern)
            .literal_separator(true)
            .build()
            .map_err(|error| format!("invalid {label} pattern `{pattern}`: {error}"))?;
        builder.add(glob);
        glob_count += 1;
    }
    let globset = if glob_count == 0 {
        None
    } else {
        Some(
            builder
                .build()
                .map_err(|error| format!("failed to compile {label} patterns: {error}"))?,
        )
    };
    Ok(CompiledPathGlobs {
        suffix_ext_any,
        suffix_any,
        globset,
    })
}

fn paired_suffix_globs(patterns: &[String]) -> Vec<String> {
    let mut suffixes = Vec::new();
    for pattern in patterns {
        let Some(suffix) = simple_glob_suffix(pattern) else {
            continue;
        };
        let has_pair = patterns.iter().any(|candidate| {
            candidate != pattern
                && simple_glob_suffix(candidate).is_some_and(|other| other == suffix)
        });
        if has_pair && !suffixes.iter().any(|existing| existing == suffix) {
            suffixes.push(suffix.to_string());
        }
    }
    suffixes
}

fn simple_glob_suffix(pattern: &str) -> Option<&str> {
    let suffix = pattern
        .strip_prefix("**/*")
        .or_else(|| pattern.strip_prefix('*'))?;
    (!suffix.is_empty()
        && !suffix
            .chars()
            .any(|character| matches!(character, '*' | '?' | '[' | ']' | '{' | '}')))
    .then_some(suffix)
}

fn simple_extension_suffix(suffix: &str) -> bool {
    suffix
        .strip_prefix('.')
        .is_some_and(|extension| !extension.is_empty() && !extension.contains('.'))
}

pub(super) fn compile_command_contains(
    patterns: Vec<String>,
) -> Result<CompiledCommandContains, String> {
    if patterns.is_empty() {
        return Ok(CompiledCommandContains::default());
    }
    AhoCorasickBuilder::new()
        .ascii_case_insensitive(true)
        .match_kind(MatchKind::LeftmostFirst)
        .build(patterns)
        .map(|matcher| CompiledCommandContains {
            matcher: Some(matcher),
        })
        .map_err(|error| format!("failed to compile commandContainsAny patterns: {error}"))
}

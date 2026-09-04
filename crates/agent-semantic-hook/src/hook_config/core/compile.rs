use std::collections::HashSet;

use super::match_types::CompiledCommandContains;
use super::match_types::CompiledPathGlobs;
use super::match_types::DurableCommandContainsMatcher;
use super::match_types::build_command_contains;
use super::match_types::build_globset;

pub(crate) fn compile_globs(
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
    let matcher_patterns = patterns
        .into_iter()
        .filter(|pattern| {
            !simple_glob_suffix(pattern)
                .is_some_and(|suffix| paired_suffixes.iter().any(|existing| existing == suffix))
        })
        .collect::<Vec<_>>();
    let matcher = build_globset(label, &matcher_patterns)?;
    Ok(CompiledPathGlobs::live(
        suffix_ext_any,
        suffix_any,
        matcher_patterns,
        matcher,
    ))
}

fn paired_suffix_globs(patterns: &[String]) -> Vec<String> {
    let mut suffixes = Vec::new();
    for pattern in patterns {
        let Some(suffix) = simple_glob_suffix(pattern) else {
            continue;
        };
        if patterns.iter().any(|candidate| {
            candidate != pattern
                && simple_glob_suffix(candidate).is_some_and(|other| other == suffix)
        }) && !suffixes.iter().any(|existing| existing == suffix)
        {
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

pub(in crate::hook_config::core) fn compile_command_contains(
    patterns: Vec<String>,
) -> Result<CompiledCommandContains, String> {
    let durable = DurableCommandContainsMatcher { patterns };
    let matcher = build_command_contains(&durable.patterns)?;
    Ok(match matcher {
        Some(matcher) => CompiledCommandContains::live(matcher, durable),
        None => CompiledCommandContains::default(),
    })
}

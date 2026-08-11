use std::collections::HashSet;

use aho_corasick::{AhoCorasickBuilder, MatchKind};
use globset::{GlobBuilder, GlobSetBuilder};
use regex_automata::dfa::dense;

use super::match_types::{
    CompiledCommandContains, CompiledPathGlobs, DurableCommandContainsMatcher, DurableDfaMatcher,
};

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
    let mut builder = GlobSetBuilder::new();
    let mut regexes = Vec::new();
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
        regexes.push(glob.regex().to_owned());
        builder.add(glob);
    }
    let globset = if regexes.is_empty() {
        None
    } else {
        Some(
            builder
                .build()
                .map_err(|error| format!("failed to compile {label} patterns: {error}"))?,
        )
    };
    let dfa = compile_dfa_union(label, &regexes)?;
    Ok(CompiledPathGlobs::live(
        suffix_ext_any,
        suffix_any,
        globset,
        dfa,
    ))
}

fn compile_dfa_union(label: &str, regexes: &[String]) -> Result<Option<DurableDfaMatcher>, String> {
    if regexes.is_empty() {
        return Ok(None);
    }
    let pattern = regexes
        .iter()
        .map(|regex| format!("(?:{regex})"))
        .collect::<Vec<_>>()
        .join("|");
    let dfa = dense::Builder::new()
        .syntax(regex_automata::util::syntax::Config::new().utf8(false))
        .build(&pattern)
        .map_err(|error| {
            format!("failed to compile durable {label} matcher `{pattern}`: {error:?}")
        })?;
    let (bytes, padding) = dfa.to_bytes_native_endian();
    DurableDfaMatcher::new(bytes, padding).map(Some)
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

pub(in crate::hook_config::core) fn compile_command_contains(
    patterns: Vec<String>,
) -> Result<CompiledCommandContains, String> {
    if patterns.is_empty() {
        return Ok(CompiledCommandContains::default());
    }
    let regexes = patterns
        .iter()
        .map(|pattern| regex_syntax::escape(pattern))
        .collect::<Vec<_>>();
    let durable = DurableCommandContainsMatcher {
        dfa: compile_ascii_case_insensitive_dfa_union("commandContainsAny", &regexes)?,
    };
    AhoCorasickBuilder::new()
        .ascii_case_insensitive(true)
        .match_kind(MatchKind::LeftmostFirst)
        .build(patterns)
        .map(|matcher| CompiledCommandContains::live(matcher, durable))
        .map_err(|error| format!("failed to compile commandContainsAny patterns: {error}"))
}

fn compile_ascii_case_insensitive_dfa_union(
    label: &str,
    regexes: &[String],
) -> Result<Option<DurableDfaMatcher>, String> {
    if regexes.is_empty() {
        return Ok(None);
    }
    let pattern = regexes
        .iter()
        .map(|regex| format!("(?:{regex})"))
        .collect::<Vec<_>>()
        .join("|");
    let dfa = dense::Builder::new()
        .syntax(
            regex_automata::util::syntax::Config::new()
                .utf8(false)
                .unicode(false)
                .case_insensitive(true),
        )
        .build(&pattern)
        .map_err(|error| format!("failed to compile durable {label} matcher: {error}"))?;
    let (bytes, padding) = dfa.to_bytes_native_endian();
    DurableDfaMatcher::new(bytes, padding).map(Some)
}

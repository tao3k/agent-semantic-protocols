// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Native ripgrep argv analysis without rewriting the admitted token stream.

use std::path::{Component, Path};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeRgDiagnosticKind {
    UnknownOption,
    MissingOptionValue,
    MissingPattern,
    ImmutableRootViolation,
    SecondaryProcessForbidden,
    NonSearchOperation,
    OutputNotAttributable,
}

impl NativeRgDiagnosticKind {
    #[must_use]
    pub const fn reason_kind(self) -> &'static str {
        match self {
            Self::UnknownOption => "search-playbook-rg-option-unknown",
            Self::MissingOptionValue => "search-playbook-rg-option-value-missing",
            Self::MissingPattern => "search-playbook-rg-pattern-missing",
            Self::ImmutableRootViolation => "search-playbook-rg-root-outside-generation",
            Self::SecondaryProcessForbidden => "search-playbook-rg-secondary-process-forbidden",
            Self::NonSearchOperation => "search-playbook-rg-operation-not-search",
            Self::OutputNotAttributable => "search-playbook-rg-output-not-attributable",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeRgDiagnostic {
    pub kind: NativeRgDiagnosticKind,
    pub argv_token_index: Option<usize>,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeRgOptionOccurrence {
    pub option: String,
    pub option_token_index: usize,
    pub value_token_index: Option<usize>,
    pub value: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeRgValueOccurrence {
    pub value: String,
    pub token_index: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeRgOutputAttribution {
    JsonPathLine,
    VimgrepPathLine,
    PathLine,
    PathOnly,
    PathOptional,
    Unattributable,
}

impl NativeRgOutputAttribution {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::JsonPathLine => "json-path-line",
            Self::VimgrepPathLine => "vimgrep-path-line",
            Self::PathLine => "path-line",
            Self::PathOnly => "path-only",
            Self::PathOptional => "path-optional",
            Self::Unattributable => "unattributable",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeRgArgvAnalysis {
    pub argv: Vec<String>,
    pub options: Vec<NativeRgOptionOccurrence>,
    pub patterns: Vec<NativeRgValueOccurrence>,
    pub search_roots: Vec<NativeRgValueOccurrence>,
    pub output_attribution: NativeRgOutputAttribution,
    pub diagnostics: Vec<NativeRgDiagnostic>,
}

impl NativeRgArgvAnalysis {
    #[must_use]
    pub fn is_admitted(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

const VALUE_LONG_OPTIONS: &[&str] = &[
    "--after-context",
    "--before-context",
    "--color",
    "--colors",
    "--context",
    "--context-separator",
    "--dfa-size-limit",
    "--encoding",
    "--engine",
    "--field-context-separator",
    "--field-match-separator",
    "--file",
    "--generate",
    "--glob",
    "--hostname-bin",
    "--hyperlink-format",
    "--iglob",
    "--ignore-file",
    "--max-columns",
    "--max-count",
    "--max-depth",
    "--max-filesize",
    "--path-separator",
    "--pre",
    "--pre-glob",
    "--regex-size-limit",
    "--regexp",
    "--replace",
    "--sort",
    "--sortr",
    "--threads",
    "--type",
    "--type-add",
    "--type-clear",
    "--type-not",
];

const BOOLEAN_LONG_OPTIONS: &[&str] = &[
    "--auto-hybrid-regex",
    "--binary",
    "--block-buffered",
    "--byte-offset",
    "--case-sensitive",
    "--column",
    "--count",
    "--count-matches",
    "--crlf",
    "--debug",
    "--files",
    "--files-with-matches",
    "--files-without-match",
    "--fixed-strings",
    "--follow",
    "--glob-case-insensitive",
    "--heading",
    "--help",
    "--hidden",
    "--ignore-case",
    "--ignore-file-case-insensitive",
    "--include-zero",
    "--invert-match",
    "--json",
    "--line-buffered",
    "--line-number",
    "--line-regexp",
    "--max-columns-preview",
    "--mmap",
    "--multiline",
    "--multiline-dotall",
    "--no-config",
    "--no-filename",
    "--no-ignore",
    "--no-ignore-dot",
    "--no-ignore-exclude",
    "--no-ignore-files",
    "--no-ignore-global",
    "--no-ignore-messages",
    "--no-ignore-parent",
    "--no-ignore-vcs",
    "--no-line-number",
    "--no-messages",
    "--no-pcre2-unicode",
    "--no-require-git",
    "--no-unicode",
    "--null",
    "--null-data",
    "--one-file-system",
    "--only-matching",
    "--passthru",
    "--pcre2",
    "--pcre2-version",
    "--pretty",
    "--quiet",
    "--search-zip",
    "--smart-case",
    "--sort-files",
    "--stats",
    "--stop-on-nonmatch",
    "--text",
    "--trace",
    "--trim",
    "--type-list",
    "--unrestricted",
    "--version",
    "--vimgrep",
    "--with-filename",
    "--word-regexp",
];

const SHORT_VALUE_OPTIONS: &[char] = &[
    'A', 'B', 'C', 'E', 'M', 'T', 'd', 'e', 'f', 'g', 'j', 'm', 'r', 't',
];
const SHORT_BOOLEAN_OPTIONS: &[char] = &[
    '.', '0', 'F', 'H', 'I', 'L', 'N', 'P', 'S', 'U', 'V', 'a', 'b', 'c', 'h', 'i', 'l', 'n', 'o',
    'p', 'q', 's', 'u', 'v', 'w', 'x', 'z',
];

/// Return whether one native rg option token consumes the following argv
/// token as its value. The PlayBook boundary parser uses the same option-arity
/// table as native rg admission so a value such as `--graph` is not mistaken
/// for a top-level PlayBook boundary.
pub(crate) fn option_consumes_following_value(token: &str) -> bool {
    if let Some(name) = token.strip_prefix("--") {
        let (name, attached) = name
            .split_once('=')
            .map_or((name, None), |(name, value)| (name, Some(value)));
        return attached.is_none() && VALUE_LONG_OPTIONS.contains(&format!("--{name}").as_str());
    }
    let Some(cluster) = token
        .strip_prefix('-')
        .filter(|cluster| !cluster.is_empty())
    else {
        return false;
    };
    for (offset, option) in cluster.char_indices() {
        if SHORT_VALUE_OPTIONS.contains(&option) {
            return offset + option.len_utf8() == cluster.len();
        }
    }
    false
}

/// Return whether one native rg option token supplies an explicit pattern (or
/// selects the pattern-free `--files` operation). This is needed only to
/// disambiguate the first positional token after native `--` from a PlayBook
/// boundary.
pub(crate) fn option_supplies_pattern(token: &str) -> bool {
    if token == "--files" {
        return true;
    }
    if let Some(name) = token.strip_prefix("--") {
        let name = name.split_once('=').map_or(name, |(name, _)| name);
        return matches!(name, "regexp" | "file");
    }
    let Some(cluster) = token
        .strip_prefix('-')
        .filter(|cluster| !cluster.is_empty())
    else {
        return false;
    };
    cluster
        .chars()
        .find(|option| SHORT_VALUE_OPTIONS.contains(option))
        .is_some_and(|option| matches!(option, 'e' | 'f'))
}

#[must_use]
pub fn analyze_native_rg_argv(argv: &[String]) -> NativeRgArgvAnalysis {
    let mut analysis = NativeRgArgvAnalysis {
        argv: argv.to_vec(),
        options: Vec::new(),
        patterns: Vec::new(),
        search_roots: Vec::new(),
        output_attribution: NativeRgOutputAttribution::PathOptional,
        diagnostics: Vec::new(),
    };
    let mut positionals = Vec::<NativeRgValueOccurrence>::new();
    let mut explicit_pattern = false;
    let mut files_mode = false;
    let mut options_done = false;
    let mut index = 0;
    while index < argv.len() {
        let token = &argv[index];
        if !options_done && token == "--" {
            options_done = true;
            index += 1;
            continue;
        }
        if !options_done && token.starts_with("--") {
            parse_long_option(
                argv,
                &mut index,
                &mut analysis,
                &mut explicit_pattern,
                &mut files_mode,
            );
            continue;
        }
        if !options_done && token.starts_with('-') && token != "-" {
            parse_short_options(argv, &mut index, &mut analysis, &mut explicit_pattern);
            continue;
        }
        positionals.push(NativeRgValueOccurrence {
            value: token.clone(),
            token_index: Some(index),
        });
        index += 1;
    }

    if explicit_pattern || files_mode {
        analysis.search_roots = positionals;
    } else if let Some((pattern, roots)) = positionals.split_first() {
        analysis.patterns.push(pattern.clone());
        analysis.search_roots.extend_from_slice(roots);
    } else {
        analysis.diagnostics.push(NativeRgDiagnostic {
            kind: NativeRgDiagnosticKind::MissingPattern,
            argv_token_index: None,
            message:
                "native rg argv requires a positional pattern, -e/--regexp, -f/--file, or --files"
                    .to_owned(),
        });
    }
    if analysis.search_roots.is_empty() {
        analysis.search_roots.push(NativeRgValueOccurrence {
            value: ".".to_owned(),
            token_index: None,
        });
    }
    for root in &analysis.search_roots {
        if root.value == "-" || !is_generation_relative(&root.value) {
            analysis.diagnostics.push(NativeRgDiagnostic {
                kind: NativeRgDiagnosticKind::ImmutableRootViolation,
                argv_token_index: root.token_index,
                message: format!(
                    "rg search root must remain inside the immutable generation: {}",
                    root.value
                ),
            });
        }
    }
    analysis.output_attribution = output_attribution(&analysis.options);
    if analysis.output_attribution == NativeRgOutputAttribution::Unattributable {
        analysis.diagnostics.push(NativeRgDiagnostic {
            kind: NativeRgDiagnosticKind::OutputNotAttributable,
            argv_token_index: None,
            message: "rg output must retain a canonical owner path for Search Layout grounding"
                .to_owned(),
        });
    }
    analysis
}

fn parse_long_option(
    argv: &[String],
    index: &mut usize,
    analysis: &mut NativeRgArgvAnalysis,
    explicit_pattern: &mut bool,
    files_mode: &mut bool,
) {
    let token_index = *index;
    let token = &argv[token_index];
    let (name, attached) = token
        .split_once('=')
        .map_or((token.as_str(), None), |(name, value)| (name, Some(value)));
    let requires_value = VALUE_LONG_OPTIONS.contains(&name);
    let known_boolean = BOOLEAN_LONG_OPTIONS.contains(&name)
        || name.strip_prefix("--no-").is_some_and(known_negatable_long);
    if !requires_value && !known_boolean {
        analysis.diagnostics.push(NativeRgDiagnostic {
            kind: NativeRgDiagnosticKind::UnknownOption,
            argv_token_index: Some(token_index),
            message: format!("native rg does not recognize option `{name}`"),
        });
        analysis.options.push(NativeRgOptionOccurrence {
            option: name.to_owned(),
            option_token_index: token_index,
            value_token_index: None,
            value: attached.map(str::to_owned),
        });
        *index += 1;
        return;
    }
    let (value, value_token_index) = if requires_value {
        if let Some(value) = attached {
            (Some(value.to_owned()), Some(token_index))
        } else if let Some(value) = argv.get(token_index + 1) {
            *index += 1;
            (Some(value.clone()), Some(token_index + 1))
        } else {
            analysis.diagnostics.push(NativeRgDiagnostic {
                kind: NativeRgDiagnosticKind::MissingOptionValue,
                argv_token_index: Some(token_index),
                message: format!("native rg option `{name}` requires a value"),
            });
            (None, None)
        }
    } else {
        if attached.is_some() {
            analysis.diagnostics.push(NativeRgDiagnostic {
                kind: NativeRgDiagnosticKind::UnknownOption,
                argv_token_index: Some(token_index),
                message: format!("native rg boolean option `{name}` does not accept a value"),
            });
        }
        (attached.map(str::to_owned), attached.map(|_| token_index))
    };
    record_option_semantics(
        name,
        value.as_deref(),
        value_token_index,
        analysis,
        explicit_pattern,
        files_mode,
    );
    analysis.options.push(NativeRgOptionOccurrence {
        option: name.to_owned(),
        option_token_index: token_index,
        value_token_index,
        value,
    });
    *index += 1;
}

fn parse_short_options(
    argv: &[String],
    index: &mut usize,
    analysis: &mut NativeRgArgvAnalysis,
    explicit_pattern: &mut bool,
) {
    let token_index = *index;
    let token = &argv[token_index];
    let cluster = &token[1..];
    for (offset, option) in cluster.char_indices() {
        let name = format!("-{option}");
        if SHORT_VALUE_OPTIONS.contains(&option) {
            let attached_start = offset + option.len_utf8();
            let attached = &cluster[attached_start..];
            let (value, value_token_index) = if attached.is_empty() {
                if let Some(value) = argv.get(token_index + 1) {
                    *index += 1;
                    (Some(value.clone()), Some(token_index + 1))
                } else {
                    analysis.diagnostics.push(NativeRgDiagnostic {
                        kind: NativeRgDiagnosticKind::MissingOptionValue,
                        argv_token_index: Some(token_index),
                        message: format!("native rg option `{name}` requires a value"),
                    });
                    (None, None)
                }
            } else {
                (Some(attached.to_owned()), Some(token_index))
            };
            let mut files_mode = false;
            record_option_semantics(
                &name,
                value.as_deref(),
                value_token_index,
                analysis,
                explicit_pattern,
                &mut files_mode,
            );
            analysis.options.push(NativeRgOptionOccurrence {
                option: name,
                option_token_index: token_index,
                value_token_index,
                value,
            });
            break;
        }
        if !SHORT_BOOLEAN_OPTIONS.contains(&option) {
            analysis.diagnostics.push(NativeRgDiagnostic {
                kind: NativeRgDiagnosticKind::UnknownOption,
                argv_token_index: Some(token_index),
                message: format!("native rg does not recognize short option `-{option}`"),
            });
        }
        let mut files_mode = false;
        record_option_semantics(
            &name,
            None,
            Some(token_index),
            analysis,
            explicit_pattern,
            &mut files_mode,
        );
        analysis.options.push(NativeRgOptionOccurrence {
            option: name,
            option_token_index: token_index,
            value_token_index: None,
            value: None,
        });
    }
    *index += 1;
}

fn record_option_semantics(
    name: &str,
    value: Option<&str>,
    value_token_index: Option<usize>,
    analysis: &mut NativeRgArgvAnalysis,
    explicit_pattern: &mut bool,
    files_mode: &mut bool,
) {
    match name {
        "-e" | "--regexp" => {
            *explicit_pattern = true;
            if let Some(value) = value {
                analysis.patterns.push(NativeRgValueOccurrence {
                    value: value.to_owned(),
                    token_index: value_token_index,
                });
            }
        }
        "-f" | "--file" => {
            *explicit_pattern = true;
            if let Some(value) = value {
                analysis.patterns.push(NativeRgValueOccurrence {
                    value: format!("@{value}"),
                    token_index: value_token_index,
                });
                if value == "-" || !is_generation_relative(value) {
                    analysis.diagnostics.push(NativeRgDiagnostic {
                        kind: NativeRgDiagnosticKind::ImmutableRootViolation,
                        argv_token_index: value_token_index,
                        message: format!(
                            "rg pattern file must remain inside the immutable generation: {value}"
                        ),
                    });
                }
            }
        }
        "--ignore-file" => {
            if let Some(value) = value
                && !is_generation_relative(value)
            {
                analysis.diagnostics.push(NativeRgDiagnostic {
                    kind: NativeRgDiagnosticKind::ImmutableRootViolation,
                    argv_token_index: value_token_index,
                    message: format!(
                        "rg ignore file must remain inside the immutable generation: {value}"
                    ),
                });
            }
        }
        "--pre" | "--pre-glob" | "--hostname-bin" | "-z" | "--search-zip" => {
            analysis.diagnostics.push(NativeRgDiagnostic {
                kind: NativeRgDiagnosticKind::SecondaryProcessForbidden,
                argv_token_index: value_token_index,
                message: format!(
                    "rg option `{name}` can escape the one-process immutable-generation boundary"
                ),
            });
        }
        "--files" => *files_mode = true,
        "-h" | "--help" | "-V" | "--version" | "--pcre2-version" | "--type-list" | "--generate" => {
            analysis.diagnostics.push(NativeRgDiagnostic {
                kind: NativeRgDiagnosticKind::NonSearchOperation,
                argv_token_index: value_token_index,
                message: format!("rg option `{name}` does not produce Search match evidence"),
            });
        }
        _ => {}
    }
}

fn output_attribution(options: &[NativeRgOptionOccurrence]) -> NativeRgOutputAttribution {
    let has = |names: &[&str]| {
        options
            .iter()
            .any(|option| names.contains(&option.option.as_str()))
    };
    if has(&["-q", "--quiet", "-I", "--no-filename", "--passthru"]) {
        NativeRgOutputAttribution::Unattributable
    } else if has(&["--json"]) {
        NativeRgOutputAttribution::JsonPathLine
    } else if has(&["--vimgrep"]) {
        NativeRgOutputAttribution::VimgrepPathLine
    } else if has(&[
        "-l",
        "--files-with-matches",
        "--files-without-match",
        "-c",
        "--count",
        "--count-matches",
        "--files",
    ]) {
        NativeRgOutputAttribution::PathOnly
    } else if has(&["-n", "--line-number"]) && !has(&["-N", "--no-line-number"]) {
        NativeRgOutputAttribution::PathLine
    } else if has(&["-H", "--with-filename"]) {
        NativeRgOutputAttribution::PathOnly
    } else {
        NativeRgOutputAttribution::PathOptional
    }
}

fn known_negatable_long(base: &str) -> bool {
    let positive = format!("--{base}");
    BOOLEAN_LONG_OPTIONS.contains(&positive.as_str())
        || VALUE_LONG_OPTIONS.contains(&positive.as_str())
}

fn is_generation_relative(value: &str) -> bool {
    let path = Path::new(value);
    !path.is_absolute()
        && !path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::RootDir))
}

#[cfg(test)]
#[path = "../tests/unit/native_rg.rs"]
mod tests;

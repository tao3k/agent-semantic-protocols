use crate::protocol_activation::{ActivatedProvider, HookRuntime};
use crate::source_selector::{
    provider_matches_source_extension, provider_matches_source_type, push_source_extension,
    selector_has_glob,
};

use super::provider_candidates::push_provider_once;
use super::shell::{command_name, is_separator};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum RawSearchCommandKind {
    RipgrepLike,
    GrepLike,
    Fd,
    Find,
    GitGrep,
    GitLsFiles,
}

#[derive(Default)]
struct RawSearchScope {
    extensions: Vec<String>,
    types: Vec<String>,
    globs: Vec<String>,
    paths: Vec<String>,
    implicit_workspace: bool,
}

pub(crate) struct RawSearchPlan<'a> {
    pub(crate) providers: Vec<&'a ActivatedProvider>,
    pub(crate) terms: Vec<String>,
}

struct ParsedRawSearch {
    scope: RawSearchScope,
    terms: Vec<String>,
}

pub(crate) fn raw_search_plan<'a>(
    registry: &'a HookRuntime,
    tokens: &[String],
) -> Option<RawSearchPlan<'a>> {
    if filters_provider_command_output(registry, tokens) {
        return None;
    }
    let parsed = ParsedRawSearch::from_tokens(tokens)?;
    Some(RawSearchPlan {
        providers: parsed.scope.matching_providers(registry),
        terms: parsed.terms,
    })
}

fn filters_provider_command_output(registry: &HookRuntime, tokens: &[String]) -> bool {
    let mut previous_stage: Option<&[String]> = None;
    let mut separator_before_stage: Option<&str> = None;
    let mut start = 0;
    while start < tokens.len() {
        while tokens.get(start).is_some_and(|token| is_separator(token)) {
            separator_before_stage = tokens.get(start).map(String::as_str);
            start += 1;
        }
        if start >= tokens.len() {
            break;
        }
        let end = tokens[start..]
            .iter()
            .position(|token| is_separator(token))
            .map(|offset| start + offset)
            .unwrap_or(tokens.len());
        let stage = &tokens[start..end];
        if separator_before_stage == Some("|")
            && raw_search_kind(stage).is_some()
            && previous_stage.is_some_and(|stage| is_provider_command_stage(registry, stage))
        {
            return true;
        }
        previous_stage = Some(stage);
        start = end;
    }
    false
}

fn is_provider_command_stage(registry: &HookRuntime, stage: &[String]) -> bool {
    let Some(command) = stage.first().map(|token| command_name(token)) else {
        return false;
    };
    registry.providers.iter().any(|provider| {
        if provider.provider_command_prefix.is_empty() {
            provider.binary == command
        } else {
            stage_matches_provider_prefix(stage, &provider.provider_command_prefix)
        }
    })
}

fn stage_matches_provider_prefix(stage: &[String], prefix: &[String]) -> bool {
    if prefix.is_empty() || stage.len() < prefix.len() {
        return false;
    }
    let Some((first, rest)) = prefix.split_first() else {
        return false;
    };
    command_name(&stage[0]) == command_name(first)
        && stage[1..prefix.len()]
            .iter()
            .zip(rest)
            .all(|(actual, expected)| actual == expected)
}

impl ParsedRawSearch {
    fn from_tokens(tokens: &[String]) -> Option<Self> {
        let (stage, kind) = raw_search_stage(tokens)?;
        let parsed = match kind {
            RawSearchCommandKind::RipgrepLike => parse_ripgrep_like(stage),
            RawSearchCommandKind::GrepLike => parse_grep_like(stage),
            RawSearchCommandKind::Fd => parse_fd(stage),
            RawSearchCommandKind::Find => parse_find(stage),
            RawSearchCommandKind::GitGrep => parse_git_grep(stage),
            RawSearchCommandKind::GitLsFiles => parse_git_ls_files(stage),
        };
        Some(parsed.normalized())
    }

    fn empty() -> Self {
        Self {
            scope: RawSearchScope::default(),
            terms: Vec::new(),
        }
    }

    fn normalized(mut self) -> Self {
        self.scope.normalize();
        self.terms = normalize_terms(self.terms);
        self
    }
}

pub(super) fn raw_search_stage(tokens: &[String]) -> Option<(&[String], RawSearchCommandKind)> {
    let mut start = 0;
    while start < tokens.len() {
        while tokens.get(start).is_some_and(|token| is_separator(token)) {
            start += 1;
        }
        if start >= tokens.len() {
            break;
        }
        let end = tokens[start..]
            .iter()
            .position(|token| is_separator(token))
            .map(|offset| start + offset)
            .unwrap_or(tokens.len());
        let stage = &tokens[start..end];
        if let Some(kind) = raw_search_kind(stage) {
            return Some((stage, kind));
        }
        start = end + 1;
    }
    None
}

fn parse_ripgrep_like(stage: &[String]) -> ParsedRawSearch {
    let mut parsed = ParsedRawSearch::empty();
    let mut positional = Vec::new();
    let mut index = 1;
    let mut files_mode = false;
    let mut pattern_from_flag = false;
    while index < stage.len() {
        let token = &stage[index];
        if token == "--" {
            positional.extend(stage[index + 1..].iter().cloned());
            break;
        }
        if token == "--files" {
            files_mode = true;
            index += 1;
            continue;
        }
        if matches!(
            token.as_str(),
            "-g" | "--glob" | "--iglob" | "-t" | "--type" | "-T" | "--type-not"
        ) {
            if let Some(value) = stage.get(index + 1) {
                if matches!(token.as_str(), "-t" | "--type") {
                    push_type(&mut parsed.scope.types, value);
                    push_source_selector(&mut parsed.scope, value, true);
                } else if !matches!(token.as_str(), "-T" | "--type-not") {
                    push_source_selector(&mut parsed.scope, value, false);
                }
            }
            index += 2;
            continue;
        }
        if matches!(token.as_str(), "-e" | "--regexp") {
            if let Some(value) = stage.get(index + 1) {
                parsed.terms.push(value.clone());
            }
            pattern_from_flag = true;
            index += 2;
            continue;
        }
        if matches!(token.as_str(), "-f" | "--file") {
            pattern_from_flag = true;
            index += 2;
            continue;
        }
        if let Some(value) = token.strip_prefix("--regexp=") {
            parsed.terms.push(value.to_string());
            pattern_from_flag = true;
            index += 1;
            continue;
        }
        if let Some(value) = token
            .strip_prefix("--glob=")
            .or_else(|| token.strip_prefix("--iglob="))
        {
            push_source_selector(&mut parsed.scope, value, false);
            index += 1;
            continue;
        }
        if let Some(value) = token.strip_prefix("--type=") {
            push_type(&mut parsed.scope.types, value);
            push_source_selector(&mut parsed.scope, value, true);
            index += 1;
            continue;
        }
        if token.starts_with("-g") && token.len() > 2 {
            push_source_selector(&mut parsed.scope, &token[2..], false);
            index += 1;
            continue;
        }
        if token.starts_with("-t") && token.len() > 2 {
            push_type(&mut parsed.scope.types, &token[2..]);
            push_source_selector(&mut parsed.scope, &token[2..], true);
            index += 1;
            continue;
        }
        if token.strip_prefix("--type-not=").is_some() {
            index += 1;
            continue;
        }
        if raw_search_option_takes_value(token) {
            index += 2;
            continue;
        }
        if token.starts_with('-') {
            index += 1;
            continue;
        }
        positional.push(token.clone());
        index += 1;
    }
    if !files_mode
        && !pattern_from_flag
        && let Some(pattern) = positional.first()
    {
        parsed.terms.push(pattern.clone());
    }
    let path_start = usize::from(!files_mode && !pattern_from_flag && !positional.is_empty());
    for path in positional.into_iter().skip(path_start) {
        parsed.scope.paths.push(path);
    }
    if parsed.scope.paths.is_empty() && !parsed.scope.has_language_filter() {
        parsed.scope.implicit_workspace = true;
    }
    parsed
}

fn parse_grep_like(stage: &[String]) -> ParsedRawSearch {
    let mut parsed = ParsedRawSearch::empty();
    let mut positional = Vec::new();
    let mut index = 1;
    let mut pattern_from_flag = false;
    while index < stage.len() {
        let token = &stage[index];
        if token == "--" {
            positional.extend(stage[index + 1..].iter().cloned());
            break;
        }
        if matches!(token.as_str(), "--include" | "--include-dir") {
            if let Some(value) = stage.get(index + 1) {
                push_source_selector(&mut parsed.scope, value, false);
            }
            index += 2;
            continue;
        }
        if matches!(token.as_str(), "-e" | "--regexp") {
            if let Some(value) = stage.get(index + 1) {
                parsed.terms.push(value.clone());
            }
            pattern_from_flag = true;
            index += 2;
            continue;
        }
        if matches!(token.as_str(), "-f" | "--file") {
            pattern_from_flag = true;
            index += 2;
            continue;
        }
        if let Some(value) = token.strip_prefix("--regexp=") {
            parsed.terms.push(value.to_string());
            pattern_from_flag = true;
            index += 1;
            continue;
        }
        if let Some(value) = token.strip_prefix("--include=") {
            push_source_selector(&mut parsed.scope, value, false);
            index += 1;
            continue;
        }
        if raw_search_option_takes_value(token) {
            index += 2;
            continue;
        }
        if token.starts_with('-') {
            index += 1;
            continue;
        }
        positional.push(token.clone());
        index += 1;
    }
    if !pattern_from_flag && let Some(pattern) = positional.first() {
        parsed.terms.push(pattern.clone());
    }
    let path_start = usize::from(!pattern_from_flag && !positional.is_empty());
    for path in positional.into_iter().skip(path_start) {
        parsed.scope.paths.push(path);
    }
    parsed
}

fn parse_git_grep(stage: &[String]) -> ParsedRawSearch {
    let Some(command_index) = git_subcommand_index(stage) else {
        return ParsedRawSearch::empty();
    };
    let mut parsed = parse_grep_like(&stage[command_index..]);
    if parsed.scope.paths.is_empty() && !parsed.scope.has_language_filter() {
        parsed.scope.implicit_workspace = true;
    }
    parsed
}

fn parse_git_ls_files(stage: &[String]) -> ParsedRawSearch {
    let mut parsed = ParsedRawSearch::empty();
    let Some(command_index) = git_subcommand_index(stage) else {
        return parsed;
    };
    let mut index = command_index + 1;
    while index < stage.len() {
        let token = &stage[index];
        if token == "--" {
            parsed
                .scope
                .paths
                .extend(stage[index + 1..].iter().cloned());
            break;
        }
        if raw_search_option_takes_value(token) {
            index += 2;
            continue;
        }
        if token.starts_with('-') {
            index += 1;
            continue;
        }
        parsed.scope.paths.push(token.clone());
        index += 1;
    }
    if parsed.scope.paths.is_empty() {
        parsed.scope.implicit_workspace = true;
    }
    parsed
}

fn parse_fd(stage: &[String]) -> ParsedRawSearch {
    let mut parsed = ParsedRawSearch::empty();
    let mut positional = Vec::new();
    let mut index = 1;
    while index < stage.len() {
        let token = &stage[index];
        if token == "--" {
            positional.extend(stage[index + 1..].iter().cloned());
            break;
        }
        if matches!(token.as_str(), "-e" | "--extension" | "--ext") {
            if let Some(value) = stage.get(index + 1) {
                push_source_selector(&mut parsed.scope, value, true);
            }
            index += 2;
            continue;
        }
        if let Some(value) = token
            .strip_prefix("--extension=")
            .or_else(|| token.strip_prefix("--ext="))
        {
            push_source_selector(&mut parsed.scope, value, true);
            index += 1;
            continue;
        }
        if raw_search_option_takes_value(token) {
            index += 2;
            continue;
        }
        if token.starts_with('-') {
            index += 1;
            continue;
        }
        positional.push(token.clone());
        index += 1;
    }
    if let Some(pattern) = positional.first() {
        push_source_selector(&mut parsed.scope, pattern, false);
        parsed.terms.push(pattern.clone());
    }
    for path in positional.into_iter().skip(1) {
        parsed.scope.paths.push(path);
    }
    if parsed.scope.paths.is_empty() && !parsed.scope.has_language_filter() {
        parsed.scope.implicit_workspace = true;
    }
    parsed
}

fn parse_find(stage: &[String]) -> ParsedRawSearch {
    let mut parsed = ParsedRawSearch::empty();
    let mut index = 1;
    while index < stage.len() {
        let token = &stage[index];
        if matches!(token.as_str(), "-name" | "-iname" | "-path" | "-ipath") {
            if let Some(value) = stage.get(index + 1) {
                push_source_selector(&mut parsed.scope, value, false);
                parsed.terms.push(value.clone());
            }
            index += 2;
            continue;
        }
        if token.starts_with('-') || matches!(token.as_str(), "(" | ")" | "!" | "-o" | "-a") {
            index += 1;
            continue;
        }
        parsed.scope.paths.push(token.clone());
        index += 1;
    }
    if parsed.scope.paths.is_empty() {
        parsed.scope.implicit_workspace = true;
    }
    parsed
}

fn normalize_terms(terms: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::new();
    for term in terms {
        for value in split_term(&term) {
            if value.len() >= 3
                && value
                    .chars()
                    .any(|character| character.is_ascii_alphabetic())
                && !normalized.iter().any(|existing| existing == &value)
            {
                normalized.push(value);
            }
            if normalized.len() >= 8 {
                return normalized;
            }
        }
    }
    normalized
}

fn split_term(term: &str) -> Vec<String> {
    term.split('|')
        .filter_map(clean_term)
        .flat_map(|term| {
            if term.contains('*') || term.contains('?') {
                filename_pattern_term(&term).into_iter().collect()
            } else {
                vec![term]
            }
        })
        .collect()
}

fn clean_term(term: &str) -> Option<String> {
    let clean = term
        .trim()
        .trim_matches(|character| {
            matches!(
                character,
                '\'' | '"' | '`' | '/' | '^' | '$' | '(' | ')' | '[' | ']' | '{' | '}'
            )
        })
        .trim();
    (!clean.is_empty()).then(|| clean.to_string())
}

fn filename_pattern_term(pattern: &str) -> Option<String> {
    let basename = pattern
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(pattern);
    let stem = basename
        .rsplit_once('.')
        .map_or(basename, |(stem, _)| stem)
        .trim_matches(|character| matches!(character, '*' | '?' | '\'' | '"' | '`' | '[' | ']'));
    clean_term(stem)
}

impl RawSearchScope {
    fn has_language_filter(&self) -> bool {
        !self.extensions.is_empty() || !self.types.is_empty() || !self.globs.is_empty()
    }

    fn normalize(&mut self) {
        self.extensions.sort();
        self.extensions.dedup();
        self.types.sort();
        self.types.dedup();
        self.globs.sort();
        self.globs.dedup();
        self.paths.sort();
        self.paths.dedup();
    }

    fn matching_providers<'a>(&self, registry: &'a HookRuntime) -> Vec<&'a ActivatedProvider> {
        let mut providers = Vec::new();
        self.push_language_filter_providers(registry, &mut providers);

        if self.has_language_filter() {
            if providers.is_empty() {
                self.push_explicit_source_path_providers(registry, &mut providers);
            }
            return providers;
        }

        if self.implicit_workspace || self.paths.iter().any(|path| is_workspace_root(path)) {
            return registry.providers.iter().collect();
        }

        self.push_path_providers(registry, &mut providers);
        providers
    }

    fn push_language_filter_providers<'a>(
        &self,
        registry: &'a HookRuntime,
        providers: &mut Vec<&'a ActivatedProvider>,
    ) {
        for provider in &registry.providers {
            if self
                .extensions
                .iter()
                .any(|extension| provider_matches_source_extension(provider, extension))
                || self
                    .types
                    .iter()
                    .any(|target_type| provider_matches_source_type(provider, target_type))
            {
                push_provider_once(providers, provider);
            }
        }
        for glob in &self.globs {
            for matched in registry.providers_for_selector(glob) {
                push_provider_once(providers, matched.provider);
            }
        }
    }

    fn push_explicit_source_path_providers<'a>(
        &self,
        registry: &'a HookRuntime,
        providers: &mut Vec<&'a ActivatedProvider>,
    ) {
        for path in &self.paths {
            for matched in registry.providers_for_selector(path) {
                push_provider_once(providers, matched.provider);
            }
        }
    }

    fn push_path_providers<'a>(
        &self,
        registry: &'a HookRuntime,
        providers: &mut Vec<&'a ActivatedProvider>,
    ) {
        for path in &self.paths {
            for matched in registry.providers_for_selector(path) {
                push_provider_once(providers, matched.provider);
            }
            for provider in registry
                .providers
                .iter()
                .filter(|provider| provider.matches_search_token(path))
            {
                push_provider_once(providers, provider);
            }
        }
    }
}

fn raw_search_kind(stage: &[String]) -> Option<RawSearchCommandKind> {
    let command = stage.first().map(String::as_str).map(command_name)?;
    match command {
        "rg" | "ag" => Some(RawSearchCommandKind::RipgrepLike),
        "grep" => Some(RawSearchCommandKind::GrepLike),
        "fd" => Some(RawSearchCommandKind::Fd),
        "find" => Some(RawSearchCommandKind::Find),
        "git" => match stage.get(git_subcommand_index(stage)?).map(String::as_str) {
            Some("grep") => Some(RawSearchCommandKind::GitGrep),
            Some("ls-files") => Some(RawSearchCommandKind::GitLsFiles),
            _ => None,
        },
        _ => None,
    }
}

fn git_subcommand_index(tokens: &[String]) -> Option<usize> {
    let mut index = 1;
    while index < tokens.len() {
        match tokens[index].as_str() {
            "-C" | "-c" | "--git-dir" | "--work-tree" => index += 2,
            value
                if value.starts_with("-C")
                    || value.starts_with("-c")
                    || value.starts_with("--git-dir=")
                    || value.starts_with("--work-tree=") =>
            {
                index += 1;
            }
            value if value.starts_with('-') => index += 1,
            _ => return Some(index),
        }
    }
    None
}

fn raw_search_option_takes_value(token: &str) -> bool {
    matches!(
        token,
        "-A" | "-B"
            | "-C"
            | "--after-context"
            | "--before-context"
            | "--context"
            | "--max-count"
            | "-m"
            | "--max-depth"
            | "--sort"
            | "--sortr"
            | "--threads"
            | "-j"
    )
}

fn is_workspace_root(path: &str) -> bool {
    matches!(path, "." | "./")
}

fn push_source_selector(scope: &mut RawSearchScope, token: &str, allow_bare_extension: bool) {
    push_source_extension(&mut scope.extensions, token, allow_bare_extension);
    if selector_has_glob(token) {
        scope.globs.push(token.to_string());
    }
}

fn push_type(types: &mut Vec<String>, token: &str) {
    let clean = token
        .trim_matches(|character| matches!(character, '\'' | '"' | ',' | ';'))
        .trim_start_matches("type:")
        .to_ascii_lowercase();
    if !clean.is_empty()
        && clean.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
    {
        types.push(clean);
    }
}

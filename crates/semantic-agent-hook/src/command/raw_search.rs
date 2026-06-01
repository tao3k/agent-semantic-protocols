use crate::protocol::{LanguageProfile, ProfileRegistry};

use super::profiles::push_profile_once;
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

pub(crate) fn profiles_for_raw_search<'a>(
    registry: &'a ProfileRegistry,
    tokens: &[String],
) -> Vec<&'a LanguageProfile> {
    let Some(scope) = RawSearchScope::from_tokens(tokens) else {
        return Vec::new();
    };
    scope.matching_profiles(registry)
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

impl RawSearchScope {
    fn from_tokens(tokens: &[String]) -> Option<Self> {
        let (stage, kind) = raw_search_stage(tokens)?;
        let mut scope = match kind {
            RawSearchCommandKind::RipgrepLike => parse_ripgrep_like_scope(stage),
            RawSearchCommandKind::GrepLike => parse_grep_like_scope(stage),
            RawSearchCommandKind::Fd => parse_fd_scope(stage),
            RawSearchCommandKind::Find => parse_find_scope(stage),
            RawSearchCommandKind::GitGrep => parse_git_grep_scope(stage),
            RawSearchCommandKind::GitLsFiles => parse_git_ls_files_scope(stage),
        };
        scope.normalize();
        Some(scope)
    }

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

    fn matching_profiles<'a>(&self, registry: &'a ProfileRegistry) -> Vec<&'a LanguageProfile> {
        let mut profiles = Vec::new();
        self.push_language_filter_profiles(registry, &mut profiles);

        if self.has_language_filter() {
            if profiles.is_empty() {
                self.push_explicit_source_path_profiles(registry, &mut profiles);
            }
            return profiles;
        }

        if self.implicit_workspace || self.paths.iter().any(|path| is_workspace_root(path)) {
            return registry.profiles.iter().collect();
        }

        self.push_path_profiles(registry, &mut profiles);
        profiles
    }

    fn push_language_filter_profiles<'a>(
        &self,
        registry: &'a ProfileRegistry,
        profiles: &mut Vec<&'a LanguageProfile>,
    ) {
        for profile in &registry.profiles {
            if self
                .extensions
                .iter()
                .any(|extension| profile_matches_extension(profile, extension))
                || self
                    .types
                    .iter()
                    .any(|target_type| profile_matches_type(profile, target_type))
            {
                push_profile_once(profiles, profile);
            }
        }
        for glob in &self.globs {
            for matched in registry.profiles_for_selector(glob) {
                push_profile_once(profiles, matched.profile);
            }
        }
    }

    fn push_explicit_source_path_profiles<'a>(
        &self,
        registry: &'a ProfileRegistry,
        profiles: &mut Vec<&'a LanguageProfile>,
    ) {
        for path in &self.paths {
            for matched in registry.profiles_for_selector(path) {
                push_profile_once(profiles, matched.profile);
            }
        }
    }

    fn push_path_profiles<'a>(
        &self,
        registry: &'a ProfileRegistry,
        profiles: &mut Vec<&'a LanguageProfile>,
    ) {
        for path in &self.paths {
            for matched in registry.profiles_for_selector(path) {
                push_profile_once(profiles, matched.profile);
            }
            for profile in registry
                .profiles
                .iter()
                .filter(|profile| profile.matches_search_token(path))
            {
                push_profile_once(profiles, profile);
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

fn parse_ripgrep_like_scope(stage: &[String]) -> RawSearchScope {
    let mut scope = RawSearchScope::default();
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
                    push_type(&mut scope.types, value);
                    push_source_selector(&mut scope, value, true);
                } else if !matches!(token.as_str(), "-T" | "--type-not") {
                    push_source_selector(&mut scope, value, false);
                }
            }
            index += 2;
            continue;
        }
        if matches!(token.as_str(), "-e" | "--regexp" | "-f" | "--file") {
            pattern_from_flag = true;
            index += 2;
            continue;
        }
        if let Some(value) = token
            .strip_prefix("--glob=")
            .or_else(|| token.strip_prefix("--iglob="))
        {
            push_source_selector(&mut scope, value, false);
            index += 1;
            continue;
        }
        if let Some(value) = token.strip_prefix("--type=") {
            push_type(&mut scope.types, value);
            push_source_selector(&mut scope, value, true);
            index += 1;
            continue;
        }
        if token.starts_with("-g") && token.len() > 2 {
            push_source_selector(&mut scope, &token[2..], false);
            index += 1;
            continue;
        }
        if token.starts_with("-t") && token.len() > 2 {
            push_type(&mut scope.types, &token[2..]);
            push_source_selector(&mut scope, &token[2..], true);
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
    let path_start = usize::from(!files_mode && !pattern_from_flag && !positional.is_empty());
    for path in positional.into_iter().skip(path_start) {
        scope.paths.push(path);
    }
    if scope.paths.is_empty() && !scope.has_language_filter() {
        scope.implicit_workspace = true;
    }
    scope
}

fn parse_grep_like_scope(stage: &[String]) -> RawSearchScope {
    let mut scope = RawSearchScope::default();
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
                push_source_selector(&mut scope, value, false);
            }
            index += 2;
            continue;
        }
        if matches!(token.as_str(), "-e" | "--regexp" | "-f" | "--file") {
            pattern_from_flag = true;
            index += 2;
            continue;
        }
        if let Some(value) = token.strip_prefix("--include=") {
            push_source_selector(&mut scope, value, false);
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
    let path_start = usize::from(!pattern_from_flag && !positional.is_empty());
    for path in positional.into_iter().skip(path_start) {
        scope.paths.push(path);
    }
    scope
}

fn parse_fd_scope(stage: &[String]) -> RawSearchScope {
    let mut scope = RawSearchScope::default();
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
                push_source_selector(&mut scope, value, true);
            }
            index += 2;
            continue;
        }
        if let Some(value) = token
            .strip_prefix("--extension=")
            .or_else(|| token.strip_prefix("--ext="))
        {
            push_source_selector(&mut scope, value, true);
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
        push_source_selector(&mut scope, pattern, false);
    }
    for path in positional.into_iter().skip(1) {
        scope.paths.push(path);
    }
    if scope.paths.is_empty() && !scope.has_language_filter() {
        scope.implicit_workspace = true;
    }
    scope
}

fn parse_find_scope(stage: &[String]) -> RawSearchScope {
    let mut scope = RawSearchScope::default();
    let mut index = 1;
    while index < stage.len() {
        let token = &stage[index];
        if matches!(token.as_str(), "-name" | "-iname" | "-path" | "-ipath") {
            if let Some(value) = stage.get(index + 1) {
                push_source_selector(&mut scope, value, false);
            }
            index += 2;
            continue;
        }
        if token.starts_with('-') || matches!(token.as_str(), "(" | ")" | "!" | "-o" | "-a") {
            index += 1;
            continue;
        }
        scope.paths.push(token.clone());
        index += 1;
    }
    if scope.paths.is_empty() {
        scope.implicit_workspace = true;
    }
    scope
}

fn parse_git_grep_scope(stage: &[String]) -> RawSearchScope {
    let mut scope = RawSearchScope::default();
    let Some(command_index) = git_subcommand_index(stage) else {
        return scope;
    };
    let mut positional = Vec::new();
    let mut index = command_index + 1;
    let mut pattern_from_flag = false;
    while index < stage.len() {
        let token = &stage[index];
        if token == "--" {
            positional.extend(stage[index + 1..].iter().cloned());
            break;
        }
        if matches!(token.as_str(), "-e" | "--regexp" | "-f" | "--file") {
            pattern_from_flag = true;
            index += 2;
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
    let path_start = usize::from(!pattern_from_flag && !positional.is_empty());
    for path in positional.into_iter().skip(path_start) {
        scope.paths.push(path);
    }
    if scope.paths.is_empty() && !scope.has_language_filter() {
        scope.implicit_workspace = true;
    }
    scope
}

fn parse_git_ls_files_scope(stage: &[String]) -> RawSearchScope {
    let mut scope = RawSearchScope::default();
    let Some(command_index) = git_subcommand_index(stage) else {
        return scope;
    };
    let mut index = command_index + 1;
    while index < stage.len() {
        let token = &stage[index];
        if token == "--" {
            for path in stage[index + 1..].iter().cloned() {
                scope.paths.push(path);
            }
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
        scope.paths.push(token.clone());
        index += 1;
    }
    if scope.paths.is_empty() {
        scope.implicit_workspace = true;
    }
    scope
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

fn profile_matches_extension(profile: &LanguageProfile, extension: &str) -> bool {
    profile
        .source_extensions
        .iter()
        .any(|source| source == extension)
}

fn profile_matches_type(profile: &LanguageProfile, target_type: &str) -> bool {
    target_type == profile.language_id
        || target_type == profile.namespace
        || profile
            .source_extensions
            .iter()
            .any(|source| source.trim_start_matches('.') == target_type)
}

fn push_source_selector(scope: &mut RawSearchScope, token: &str, allow_bare_extension: bool) {
    push_extension(&mut scope.extensions, token, allow_bare_extension);
    if selector_has_glob(token) {
        scope.globs.push(token.to_string());
    }
}

fn selector_has_glob(token: &str) -> bool {
    token
        .chars()
        .any(|character| matches!(character, '*' | '?' | '[' | ']' | '{' | '}'))
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

fn push_extension(extensions: &mut Vec<String>, token: &str, allow_bare: bool) {
    let clean = token
        .trim_matches(|character| matches!(character, '\'' | '"' | ',' | ';'))
        .trim_start_matches('*')
        .to_ascii_lowercase();
    if let Some(start) = clean.find(".{") {
        if let Some(end) = clean[start + 2..].find('}') {
            for extension in clean[start + 2..start + 2 + end].split(',') {
                if !extension.is_empty()
                    && extension
                        .chars()
                        .all(|character| character.is_ascii_alphanumeric())
                {
                    extensions.push(format!(".{extension}"));
                }
            }
            return;
        }
    }
    let clean = clean.trim_start_matches('{').trim_end_matches('}');
    if allow_bare
        && clean
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
    {
        extensions.push(format!(".{clean}"));
        return;
    }
    if let Some((_, extension)) = clean.rsplit_once('.') {
        let extension = extension.trim_end_matches('}');
        if !extension.is_empty()
            && extension
                .chars()
                .all(|character| character.is_ascii_alphanumeric())
        {
            extensions.push(format!(".{extension}"));
        }
    }
}

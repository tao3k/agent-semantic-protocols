//! Runtime provider status rendering and source selector matching.

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};

use crate::protocol::normalize_source_selector;

use super::protocol_activation_manifest::{
    ActivatedProvider, HookRuntime, ProviderRoutePathContext, ProviderSelectorMatch,
    SourceSelectorKind,
};

use crate::protocol::{CommandTemplate, DecisionRoute, DecisionRouteKind};

impl HookRuntime {
    pub(crate) fn providers_for_selector(&self, selector: &str) -> Vec<ProviderSelectorMatch<'_>> {
        let matcher = SourceSelectorMatcher::new(selector);
        self.providers
            .iter()
            .filter_map(|provider| {
                provider
                    .match_source_selector_with(&matcher)
                    .map(|kind| ProviderSelectorMatch { provider, kind })
            })
            .collect()
    }
}

impl ActivatedProvider {
    fn matches_source_selector(&self, selector: &str) -> bool {
        self.match_source_selector(selector).is_some()
    }

    fn match_source_selector(&self, selector: &str) -> Option<SourceSelectorKind> {
        let matcher = SourceSelectorMatcher::new(selector);
        self.match_source_selector_with(&matcher)
    }

    fn match_source_selector_with(
        &self,
        selector: &SourceSelectorMatcher<'_>,
    ) -> Option<SourceSelectorKind> {
        if self
            .ignored_path_prefixes
            .iter()
            .any(|prefix| selector.is_ignored_by(prefix))
        {
            return None;
        }
        if self.glob_matches_source_selector(selector) {
            return Some(if selector.has_glob {
                SourceSelectorKind::Pattern
            } else {
                SourceSelectorKind::ExactPath
            });
        }
        if self
            .config_files
            .iter()
            .any(|config| selector.normalized.ends_with(config))
        {
            return Some(SourceSelectorKind::ExactPath);
        }
        None
    }

    pub(crate) fn matches_search_token(&self, token: &str) -> bool {
        let normalized = normalize_route_path(token);
        self.matches_source_selector(&normalized)
            || self.matches_source_directory_token(&normalized)
    }

    fn matches_source_directory_token(&self, normalized: &str) -> bool {
        self.source_root_matches_search_token(normalized)
            || self.package_roots_match_search_token(normalized)
    }

    fn package_roots_match_search_token(&self, normalized: &str) -> bool {
        self.package_roots
            .iter()
            .any(|root| self.package_root_matches_search_token(root, normalized))
    }

    fn package_root_matches_search_token(&self, package_root: &str, normalized: &str) -> bool {
        let package_root = normalize_route_path(package_root);
        let package_root = package_root.trim_end_matches('/');
        if package_root.is_empty() || package_root == "." {
            return false;
        }
        if normalized == package_root {
            return true;
        }
        normalized
            .strip_prefix(&format!("{package_root}/"))
            .is_some_and(|relative| self.source_root_matches_search_token(relative))
    }

    fn source_root_matches_search_token(&self, normalized: &str) -> bool {
        self.source_roots.iter().any(|root| {
            let root = normalize_route_path(root);
            let root = root.trim_end_matches('/');
            !root.is_empty() && (normalized == root || normalized.starts_with(&format!("{root}/")))
        })
    }

    fn glob_matches_source_selector(&self, selector: &SourceSelectorMatcher<'_>) -> bool {
        self.source_extensions
            .iter()
            .any(|extension| selector.targets_extension(extension))
    }

    pub(crate) fn route_from_template(
        &self,
        kind: DecisionRouteKind,
        template: &CommandTemplate,
        path: Option<&str>,
        query: Option<&str>,
    ) -> DecisionRoute {
        let route_context = path.map(|path| self.route_path_context(path));
        let route_path = route_context
            .as_ref()
            .map(|context| context.selector.as_str())
            .or(path)
            .unwrap_or("");
        let project_root = route_context
            .as_ref()
            .map(|context| context.project_root.as_str())
            .unwrap_or_else(|| self.default_route_project_root());
        let argv: Vec<String> = template
            .argv
            .iter()
            .map(|arg| {
                arg.replace("{owner}", route_path)
                    .replace("{query}", query.unwrap_or(""))
                    .replace("{workspace}", project_root)
            })
            .collect();
        let argv = self.agent_facade_argv_from_provider_argv(argv);
        DecisionRoute {
            language_id: self.language_id.clone(),
            provider_id: self.provider_id.clone(),
            binary: "asp".to_string(),
            kind,
            argv,
            stdin_mode: template.stdin_mode,
        }
    }

    pub(crate) fn agent_facade_argv<I, S>(&self, args: I) -> Vec<String>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut argv = vec!["asp".to_string(), self.language_id.clone()];
        argv.extend(args.into_iter().map(Into::into));
        argv
    }

    pub(crate) fn agent_facade_argv_from_provider_argv(&self, argv: Vec<String>) -> Vec<String> {
        if argv.first().is_some_and(|command| command == "asp") {
            return argv;
        }
        if argv.first().is_some_and(|command| command == &self.binary) {
            return self.agent_facade_argv(argv.into_iter().skip(1));
        }
        if !self.provider_command_prefix.is_empty()
            && argv.starts_with(&self.provider_command_prefix)
        {
            return self
                .agent_facade_argv(argv.into_iter().skip(self.provider_command_prefix.len()));
        }
        argv
    }

    pub(crate) fn route_path_context(&self, path: &str) -> ProviderRoutePathContext {
        let normalized = normalize_route_path(path);
        for root in self.package_roots_by_specificity() {
            if root == "." {
                continue;
            }
            if normalized == root {
                return ProviderRoutePathContext {
                    selector: ".".to_string(),
                    project_root: root,
                };
            }
            if let Some(selector) = normalized.strip_prefix(&format!("{root}/")) {
                return ProviderRoutePathContext {
                    selector: selector.to_string(),
                    project_root: root,
                };
            }
        }
        ProviderRoutePathContext {
            selector: normalized,
            project_root: self.default_route_project_root().to_string(),
        }
    }

    fn default_route_project_root(&self) -> &str {
        self.package_roots
            .iter()
            .find(|root| root.as_str() == ".")
            .map(String::as_str)
            .or_else(|| self.package_roots.first().map(String::as_str))
            .unwrap_or(".")
    }

    fn package_roots_by_specificity(&self) -> Vec<String> {
        let mut roots = self.package_roots.clone();
        roots.sort_by(|left, right| right.len().cmp(&left.len()).then(left.cmp(right)));
        roots
    }
}

fn normalize_route_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_string()
}

struct SourceSelectorMatcher<'a> {
    normalized: &'a str,
    has_glob: bool,
    extension_glob: Option<GlobSet>,
}

impl<'a> SourceSelectorMatcher<'a> {
    fn new(selector: &'a str) -> Self {
        let normalized = normalize_source_selector(selector);
        let has_glob = selector_has_glob(normalized);
        let basename = basename_pattern(normalized).to_ascii_lowercase();
        let extension_pattern = basename_extension_pattern(&basename);
        Self {
            normalized,
            has_glob,
            extension_glob: extension_pattern.and_then(build_glob_set),
        }
    }

    fn is_ignored_by(&self, prefix: &str) -> bool {
        self.normalized == prefix || self.normalized.starts_with(&format!("{prefix}/"))
    }

    fn targets_extension(&self, extension: &str) -> bool {
        let extension = extension.trim_start_matches('.').to_ascii_lowercase();
        self.extension_glob
            .as_ref()
            .is_some_and(|glob_set| glob_set.is_match(extension))
    }
}

fn build_glob_set(pattern: &str) -> Option<GlobSet> {
    let glob = GlobBuilder::new(pattern)
        .literal_separator(false)
        .backslash_escape(false)
        .build()
        .ok()?;
    let mut builder = GlobSetBuilder::new();
    builder.add(glob);
    builder.build().ok()
}

fn basename_pattern(selector: &str) -> &str {
    selector
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(selector)
}

fn basename_extension_pattern(basename: &str) -> Option<&str> {
    let (_, _, last_literal_dot) = basename.char_indices().fold(
        (0usize, 0usize, None),
        |(bracket_depth, brace_depth, last_literal_dot), (index, character)| match character {
            '[' => (bracket_depth + 1, brace_depth, last_literal_dot),
            ']' if bracket_depth > 0 => (bracket_depth - 1, brace_depth, last_literal_dot),
            '{' if bracket_depth == 0 => (bracket_depth, brace_depth + 1, last_literal_dot),
            '}' if bracket_depth == 0 && brace_depth > 0 => {
                (bracket_depth, brace_depth - 1, last_literal_dot)
            }
            '.' if bracket_depth == 0 && brace_depth == 0 => {
                (bracket_depth, brace_depth, Some(index))
            }
            _ => (bracket_depth, brace_depth, last_literal_dot),
        },
    );
    let start = last_literal_dot? + 1;
    (start < basename.len()).then_some(&basename[start..])
}

fn selector_has_glob(path: &str) -> bool {
    path.chars()
        .any(|character| matches!(character, '*' | '?' | '[' | ']' | '{' | '}'))
}

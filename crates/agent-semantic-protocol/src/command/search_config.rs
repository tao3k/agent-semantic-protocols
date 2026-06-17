//! ASP facade configuration for language routing and cheap search.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub(super) struct AspConfig {
    pub(super) search: SearchConfig,
    languages: HashMap<String, LanguageConfig>,
}

#[derive(Debug, Clone)]
pub(super) struct SearchConfig {
    pub(super) ignore_dirs: Vec<String>,
    pub(super) include_hidden_dirs: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct LanguageConfig {
    enabled: Option<bool>,
    bin: Option<String>,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            ignore_dirs: vec![
                ".cache".to_string(),
                ".codex".to_string(),
                ".data".to_string(),
                ".devenv".to_string(),
                ".direnv".to_string(),
                ".git".to_string(),
                ".idea".to_string(),
                ".jj".to_string(),
                ".run".to_string(),
                ".vscode".to_string(),
                "target".to_string(),
                "node_modules".to_string(),
                "dist".to_string(),
                "build".to_string(),
                ".build".to_string(),
                "__pycache__".to_string(),
                ".venv".to_string(),
                "venv".to_string(),
                "vendor".to_string(),
            ],
            include_hidden_dirs: Vec::new(),
        }
    }
}

impl AspConfig {
    pub(super) fn load(invocation_root: &Path, activation_root: &Path) -> Self {
        let mut config = Self::default();
        for root in config_roots(invocation_root, activation_root) {
            let path = root.join("asp.toml");
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            config.merge_text(&text);
        }
        config
    }

    pub(super) fn language_enabled(&self, language_id: &str) -> bool {
        self.languages
            .get(language_id)
            .and_then(|language| language.enabled)
            .unwrap_or(true)
    }

    pub(super) fn provider_bin(&self, language_id: &str) -> Option<&str> {
        self.languages
            .get(language_id)
            .and_then(|language| language.bin.as_deref())
            .filter(|bin| !bin.is_empty())
    }

    fn merge_text(&mut self, text: &str) {
        let mut section = ConfigSection::Root;
        for raw_line in text.lines() {
            let line = raw_line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                section = ConfigSection::parse(&line[1..line.len() - 1]);
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            self.assign(&section, key.trim(), value.trim());
        }
    }

    fn assign(&mut self, section: &ConfigSection, key: &str, value: &str) {
        match section {
            ConfigSection::Discovery | ConfigSection::Search => match key {
                "ignoredDirNames" | "ignored_dir_names" | "ignoreDirs" | "ignore_dirs" => {
                    self.search.ignore_dirs = parse_string_array(value);
                }
                "includeHiddenDirNames"
                | "include_hidden_dir_names"
                | "includeHiddenDirs"
                | "include_hidden_dirs" => {
                    self.search.include_hidden_dirs = parse_string_array(value);
                }
                _ => {}
            },
            ConfigSection::Language(language_id) => {
                let language = self.languages.entry(language_id.clone()).or_default();
                match key {
                    "enabled" => language.enabled = parse_bool(value),
                    "bin" | "binary" => language.bin = parse_string(value),
                    _ => {}
                }
            }
            ConfigSection::Root => {}
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum ConfigSection {
    Root,
    Discovery,
    Search,
    Language(String),
}

impl ConfigSection {
    fn parse(section: &str) -> Self {
        if section == "discovery" {
            return Self::Discovery;
        }
        if section == "search" {
            return Self::Search;
        }
        if let Some(language_id) = section.strip_prefix("languages.") {
            return Self::Language(language_id.trim().to_string());
        }
        if let Some(language_id) = section.strip_prefix("providers.") {
            return Self::Language(language_id.trim().to_string());
        }
        Self::Root
    }
}

fn config_roots<'a>(invocation_root: &'a Path, activation_root: &'a Path) -> Vec<&'a Path> {
    if invocation_root == activation_root {
        vec![activation_root]
    } else {
        vec![activation_root, invocation_root]
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn parse_string(value: &str) -> Option<String> {
    let value = value.trim();
    if let Some(stripped) = value
        .strip_prefix('"')
        .and_then(|text| text.strip_suffix('"'))
    {
        return Some(stripped.to_string());
    }
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn parse_string_array(value: &str) -> Vec<String> {
    let Some(inner) = value
        .trim()
        .strip_prefix('[')
        .and_then(|text| text.strip_suffix(']'))
    else {
        return Vec::new();
    };
    inner
        .split(',')
        .filter_map(parse_string)
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

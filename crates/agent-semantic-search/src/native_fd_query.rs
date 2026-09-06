//! Provider-neutral execution of the admitted fd V1 filter grammar over the
//! immutable Runtime owner inventory.

use globset::Glob;
use regex::RegexBuilder;
use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeFdAxisResult {
    pub candidate_owner_paths: Vec<String>,
    pub branch_candidate_owner_paths: Vec<Vec<String>>,
}

pub fn execute_native_fd_blocks(
    blocks: &[Vec<String>],
    owner_paths: &[String],
) -> Result<NativeFdAxisResult, String> {
    if blocks.is_empty() {
        return Err("fd axis requires at least one native argv block".to_owned());
    }
    let mut branch_candidate_owner_paths = Vec::with_capacity(blocks.len());
    for block in blocks {
        let query = NativeFdQuery::parse(block)?;
        branch_candidate_owner_paths.push(query.execute(owner_paths)?);
    }
    let mut candidate_owner_paths = branch_candidate_owner_paths
        .iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>();
    candidate_owner_paths.sort();
    candidate_owner_paths.dedup();
    Ok(NativeFdAxisResult {
        candidate_owner_paths,
        branch_candidate_owner_paths,
    })
}

struct NativeFdQuery {
    pattern: String,
    roots: Vec<String>,
    extensions: Vec<String>,
    full_path: bool,
    glob: bool,
}

impl NativeFdQuery {
    fn parse(argv: &[String]) -> Result<Self, String> {
        let mut extensions = Vec::new();
        let mut positionals = Vec::new();
        let mut full_path = false;
        let mut glob = false;
        let mut index = 0;
        let mut options = true;
        while index < argv.len() {
            let argument = &argv[index];
            if options && argument == "--" {
                options = false;
                index += 1;
                continue;
            }
            if options && matches!(argument.as_str(), "-t" | "--type") {
                let value = argv
                    .get(index + 1)
                    .ok_or_else(|| "fd --type requires a value".to_owned())?;
                if value != "f" && value != "file" {
                    return Err(
                        "resident fd inventory admits only the native file type filter".to_owned(),
                    );
                }
                index += 2;
                continue;
            }
            if options && matches!(argument.as_str(), "-e" | "--extension") {
                let value = argv
                    .get(index + 1)
                    .ok_or_else(|| "fd --extension requires a value".to_owned())?;
                extensions.push(value.trim_start_matches('.').to_owned());
                index += 2;
                continue;
            }
            if options && argument.starts_with("--extension=") {
                extensions.push(
                    argument
                        .trim_start_matches("--extension=")
                        .trim_start_matches('.')
                        .to_owned(),
                );
                index += 1;
                continue;
            }
            if options && matches!(argument.as_str(), "-p" | "--full-path") {
                full_path = true;
                index += 1;
                continue;
            }
            if options && matches!(argument.as_str(), "-g" | "--glob") {
                glob = true;
                index += 1;
                continue;
            }
            if options && argument.starts_with('-') {
                return Err(format!(
                    "fd native option is not admitted by the resident inventory parser: {argument}"
                ));
            }
            positionals.push(argument.clone());
            index += 1;
        }
        if extensions.iter().any(String::is_empty) {
            return Err("fd extension cannot be empty".to_owned());
        }
        let pattern = positionals
            .first()
            .cloned()
            .unwrap_or_else(|| ".".to_owned());
        let roots = positionals
            .get(1..)
            .unwrap_or_default()
            .iter()
            .map(|root| normalize_root(root))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            pattern,
            roots,
            extensions,
            full_path,
            glob,
        })
    }

    fn execute(&self, owner_paths: &[String]) -> Result<Vec<String>, String> {
        let matcher = if self.glob {
            NativeFdMatcher::Glob(
                Glob::new(&self.pattern)
                    .map_err(|error| format!("invalid fd glob: {error}"))?
                    .compile_matcher(),
            )
        } else {
            NativeFdMatcher::Regex(
                RegexBuilder::new(&self.pattern)
                    .case_insensitive(!self.pattern.chars().any(char::is_uppercase))
                    .build()
                    .map_err(|error| format!("invalid fd regex: {error}"))?,
            )
        };
        let mut matches = owner_paths
            .iter()
            .filter(|owner| {
                self.roots.is_empty()
                    || self.roots.iter().any(|root| {
                        root.is_empty()
                            || owner.as_str() == root
                            || owner.starts_with(&format!("{root}/"))
                    })
            })
            .filter(|owner| {
                self.extensions.is_empty()
                    || Path::new(owner)
                        .extension()
                        .and_then(|value| value.to_str())
                        .is_some_and(|value| self.extensions.iter().any(|item| item == value))
            })
            .filter(|owner| {
                let subject = if self.full_path {
                    owner.as_str()
                } else {
                    Path::new(owner)
                        .file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or(owner)
                };
                matcher.is_match(subject)
            })
            .cloned()
            .collect::<Vec<_>>();
        matches.sort();
        matches.dedup();
        Ok(matches)
    }
}

enum NativeFdMatcher {
    Regex(regex::Regex),
    Glob(globset::GlobMatcher),
}

impl NativeFdMatcher {
    fn is_match(&self, value: &str) -> bool {
        match self {
            Self::Regex(regex) => regex.is_match(value),
            Self::Glob(glob) => glob.is_match(value),
        }
    }
}

fn normalize_root(root: &str) -> Result<String, String> {
    let root = root.trim_end_matches('/');
    if root == "." || root.is_empty() {
        return Ok(String::new());
    }
    if root.starts_with('/') || root.split('/').any(|component| component == "..") {
        return Err("fd roots must stay inside the admitted workspace".to_owned());
    }
    Ok(root.trim_start_matches("./").to_owned())
}

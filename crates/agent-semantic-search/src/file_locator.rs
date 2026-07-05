//! File path locator indexes for search planning hot paths.
//!
//! The locator answers filename and path questions without provider processes
//! or source-content scans. Cold refresh backends may rebuild this index, but
//! hot lookup stays in memory and only touches indexed hit buckets.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use globset::{Glob, GlobSetBuilder};

const DEFAULT_LIMIT: usize = 16;

/// In-memory index for workspace-relative file path lookup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileLocatorIndex {
    entries: Vec<FileLocatorEntry>,
    exact_path: HashMap<String, Vec<usize>>,
    basename: HashMap<String, Vec<usize>>,
    stem: HashMap<String, Vec<usize>>,
    extension: HashMap<String, Vec<usize>>,
    segment: HashMap<String, Vec<usize>>,
    suffix_path: HashMap<String, Vec<usize>>,
    trigrams: HashMap<String, Vec<usize>>,
}

/// User-facing file locator query with a bounded result limit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileLocatorQuery {
    text: String,
    limit: usize,
}

/// Ranked file locator candidate returned by the hot path index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileLocatorCandidate {
    /// Workspace-relative path matched by the locator.
    pub workspace_relative_path: String,
    /// Algorithm class that produced this candidate.
    pub match_kind: FileLocatorMatchKind,
    /// Higher scores rank earlier in the candidate list.
    pub score: u16,
    /// Backend that produced the candidate.
    pub backend: FileLocatorBackend,
}

/// Path matching algorithm used for a file locator candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileLocatorMatchKind {
    /// Query matched the complete normalized workspace-relative path.
    ExactPath,
    /// Query matched the file basename.
    Basename,
    /// Query matched the basename without extension.
    Stem,
    /// Query matched one of the indexed path suffixes.
    SuffixPath,
    /// Query matched a directory or filename path segment.
    Segment,
    /// Query matched the file extension.
    Extension,
    /// Query matched a `globset` pattern.
    Glob,
    /// Query matched filename trigrams.
    FuzzyFilename,
}

/// Backend used to produce a file locator candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileLocatorBackend {
    /// Candidate was produced by an in-memory path index.
    InMemoryPathIndex,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileLocatorEntry {
    workspace_relative_path: String,
    normalized_path: String,
}

impl FileLocatorIndex {
    /// Build a hot lookup index from workspace-relative paths.
    #[must_use]
    pub fn build(paths: impl IntoIterator<Item = PathBuf>) -> Self {
        let mut index = Self {
            entries: Vec::new(),
            exact_path: HashMap::new(),
            basename: HashMap::new(),
            stem: HashMap::new(),
            extension: HashMap::new(),
            segment: HashMap::new(),
            suffix_path: HashMap::new(),
            trigrams: HashMap::new(),
        };

        for path in paths {
            let display_path = display_path(&path);
            let normalized_path = normalize_path(&display_path);
            if normalized_path.is_empty() {
                continue;
            }

            let entry_index = index.entries.len();
            index.entries.push(FileLocatorEntry {
                workspace_relative_path: display_path,
                normalized_path: normalized_path.clone(),
            });
            index_path(&mut index.exact_path, &normalized_path, entry_index);

            if let Some(basename) = basename(&normalized_path) {
                index_path(&mut index.basename, basename, entry_index);
                if let Some(stem) = stem(basename) {
                    index_path(&mut index.stem, stem, entry_index);
                }
                if let Some(extension) = extension(basename) {
                    index_path(&mut index.extension, extension, entry_index);
                }
                for trigram in trigrams(basename) {
                    index_path(&mut index.trigrams, &trigram, entry_index);
                }
            }

            for segment in normalized_path
                .split('/')
                .filter(|segment| !segment.is_empty())
            {
                index_path(&mut index.segment, segment, entry_index);
            }
            index_suffixes(&mut index.suffix_path, &normalized_path, entry_index);
        }

        index
    }

    /// Locate ranked path candidates for a user query.
    #[must_use]
    pub fn locate(&self, query: &FileLocatorQuery) -> Vec<FileLocatorCandidate> {
        let normalized_query = normalize_query(&query.text);
        if normalized_query.is_empty() {
            return Vec::new();
        }

        if looks_like_glob(&normalized_query) {
            return self.locate_glob(&normalized_query, query.limit);
        }

        if let Some(indices) = self.exact_path.get(&normalized_query) {
            return self.candidates_from_indices(
                indices,
                FileLocatorMatchKind::ExactPath,
                query.limit,
            );
        }

        let mut candidates = HashMap::new();
        self.collect_index_matches(
            &mut candidates,
            &self.basename,
            &normalized_query,
            FileLocatorMatchKind::Basename,
        );
        self.collect_index_matches(
            &mut candidates,
            &self.stem,
            &normalized_query,
            FileLocatorMatchKind::Stem,
        );
        self.collect_index_matches(
            &mut candidates,
            &self.suffix_path,
            &normalized_query,
            FileLocatorMatchKind::SuffixPath,
        );
        self.collect_index_matches(
            &mut candidates,
            &self.segment,
            &normalized_query,
            FileLocatorMatchKind::Segment,
        );
        self.collect_index_matches(
            &mut candidates,
            &self.extension,
            &normalized_query,
            FileLocatorMatchKind::Extension,
        );

        if candidates.is_empty() && normalized_query.len() >= 3 {
            self.collect_fuzzy_matches(&mut candidates, &normalized_query);
        }

        ranked_candidates(candidates, query.limit)
    }

    fn locate_glob(&self, pattern: &str, limit: usize) -> Vec<FileLocatorCandidate> {
        let mut builder = GlobSetBuilder::new();
        if let Ok(glob) = Glob::new(pattern) {
            builder.add(glob);
        }
        let Ok(glob_set) = builder.build() else {
            return Vec::new();
        };

        self.entries
            .iter()
            .filter(|entry| glob_set.is_match(&entry.normalized_path))
            .take(limit)
            .map(|entry| FileLocatorCandidate {
                workspace_relative_path: entry.workspace_relative_path.clone(),
                match_kind: FileLocatorMatchKind::Glob,
                score: score_for_match_kind(FileLocatorMatchKind::Glob),
                backend: FileLocatorBackend::InMemoryPathIndex,
            })
            .collect()
    }

    fn candidates_from_indices(
        &self,
        indices: &[usize],
        match_kind: FileLocatorMatchKind,
        limit: usize,
    ) -> Vec<FileLocatorCandidate> {
        indices
            .iter()
            .take(limit)
            .filter_map(|index| self.candidate(*index, match_kind))
            .collect()
    }

    fn collect_index_matches(
        &self,
        candidates: &mut HashMap<usize, FileLocatorCandidate>,
        index: &HashMap<String, Vec<usize>>,
        query: &str,
        match_kind: FileLocatorMatchKind,
    ) {
        if let Some(indices) = index.get(query) {
            self.collect_candidates(candidates, indices, match_kind);
        }
    }

    fn collect_fuzzy_matches(
        &self,
        candidates: &mut HashMap<usize, FileLocatorCandidate>,
        query: &str,
    ) {
        let query_trigrams = trigrams(query);
        let mut buckets = query_trigrams
            .iter()
            .filter_map(|trigram| {
                self.trigrams
                    .get(trigram)
                    .map(|indices| (trigram.as_str(), indices))
            })
            .collect::<Vec<_>>();
        if buckets.is_empty() {
            return;
        }

        buckets.sort_by_key(|(_, indices)| indices.len());
        let anchor = buckets[0].1;
        if anchor.len() > 2_048 {
            return;
        }

        let mut ranked_indices = anchor
            .iter()
            .map(|index| {
                let count = buckets
                    .iter()
                    .filter(|(_, indices)| indices.binary_search(index).is_ok())
                    .count();
                (*index, count)
            })
            .collect::<Vec<_>>();
        ranked_indices
            .sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
        for (index, _count) in ranked_indices.into_iter().filter(|(_, count)| *count > 0) {
            if let Some(candidate) = self.candidate(index, FileLocatorMatchKind::FuzzyFilename) {
                candidates
                    .entry(index)
                    .and_modify(|existing| {
                        if candidate.score > existing.score {
                            *existing = candidate.clone();
                        }
                    })
                    .or_insert(candidate);
            }
        }
    }

    fn collect_candidates(
        &self,
        candidates: &mut HashMap<usize, FileLocatorCandidate>,
        indices: &[usize],
        match_kind: FileLocatorMatchKind,
    ) {
        for index in indices {
            if let Some(candidate) = self.candidate(*index, match_kind) {
                candidates
                    .entry(*index)
                    .and_modify(|existing| {
                        if candidate.score > existing.score {
                            *existing = candidate.clone();
                        }
                    })
                    .or_insert(candidate);
            }
        }
    }

    fn candidate(
        &self,
        index: usize,
        match_kind: FileLocatorMatchKind,
    ) -> Option<FileLocatorCandidate> {
        self.entries.get(index).map(|entry| FileLocatorCandidate {
            workspace_relative_path: entry.workspace_relative_path.clone(),
            match_kind,
            score: score_for_match_kind(match_kind),
            backend: FileLocatorBackend::InMemoryPathIndex,
        })
    }
}

impl FileLocatorQuery {
    /// Create a bounded file locator query from user text.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            limit: DEFAULT_LIMIT,
        }
    }

    /// Override the maximum number of returned candidates.
    #[must_use]
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit.max(1);
        self
    }
}

fn ranked_candidates(
    candidates: HashMap<usize, FileLocatorCandidate>,
    limit: usize,
) -> Vec<FileLocatorCandidate> {
    let mut ranked = candidates.into_values().collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        right.score.cmp(&left.score).then_with(|| {
            left.workspace_relative_path
                .cmp(&right.workspace_relative_path)
        })
    });
    ranked.truncate(limit);
    ranked
}

fn score_for_match_kind(match_kind: FileLocatorMatchKind) -> u16 {
    match match_kind {
        FileLocatorMatchKind::ExactPath => 1000,
        FileLocatorMatchKind::Basename => 900,
        FileLocatorMatchKind::Stem => 850,
        FileLocatorMatchKind::SuffixPath => 800,
        FileLocatorMatchKind::Glob => 760,
        FileLocatorMatchKind::Segment => 650,
        FileLocatorMatchKind::Extension => 500,
        FileLocatorMatchKind::FuzzyFilename => 300,
    }
}

fn index_path(index: &mut HashMap<String, Vec<usize>>, key: &str, entry_index: usize) {
    index.entry(key.to_string()).or_default().push(entry_index);
}

fn index_suffixes(index: &mut HashMap<String, Vec<usize>>, path: &str, entry_index: usize) {
    let segments = path.split('/').collect::<Vec<_>>();
    for start in 0..segments.len() {
        let suffix = segments[start..].join("/");
        index_path(index, &suffix, entry_index);
    }
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn normalize_query(query: &str) -> String {
    normalize_path(query.trim())
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
        .trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("/")
        .to_lowercase()
}

fn basename(path: &str) -> Option<&str> {
    path.rsplit('/').find(|part| !part.is_empty())
}

fn stem(basename: &str) -> Option<&str> {
    basename
        .rfind('.')
        .filter(|index| *index > 0)
        .map(|index| &basename[..index])
}

fn extension(basename: &str) -> Option<&str> {
    basename
        .rfind('.')
        .and_then(|index| basename.get(index + 1..))
        .filter(|extension| !extension.is_empty())
}

fn looks_like_glob(query: &str) -> bool {
    query.contains('*') || query.contains('?') || query.contains('[')
}

fn trigrams(text: &str) -> Vec<String> {
    let characters = text.chars().collect::<Vec<_>>();
    if characters.len() < 3 {
        return vec![text.to_string()];
    }

    let mut values = characters
        .windows(3)
        .map(|window| window.iter().collect::<String>())
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

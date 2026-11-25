// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Language-agnostic open EDB (Extensional Database) extraction framework for
//! [`WorkspaceSearchIndex`].
//!
//! ASP's Rust layer owns only sub-millisecond trigram/topology pre-filtering and
//! generic rule-driven fact projection; all language-specific facet and edge
//! rules (Rust, Gerbil Scheme POO, SysML, Quint, Org, etc.) are owned by
//! Marlin's Scheme POO fragments and supplied as typed [`WorkspaceEdbEntityRule`]
//! / [`WorkspaceEdbEdgeRule`] descriptors.

use std::time::Instant;

use regex::Regex;

use crate::SearchGenerationDigest;
use crate::trigram_query::TrigramQuery;

use super::index::{IndexedFile, WorkspaceSearchIndex};
use super::query::CompiledFilters;

const DEFAULT_MAX_EDB_FILES: usize = 256;
const DEFAULT_MAX_EDB_FACTS: usize = 4096;

/// Declarative, host-supplied rule projecting matching lines into open
/// [`WorkspaceEdbEntityFact`] rows.
///
/// Named regex capture `name` populates `entity.name`; every other named
/// capture populates an open `(key, value)` entry in `entity.attrs`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceEdbEntityRule {
    pub language: Option<String>,
    pub facet_tag: String,
    pub pattern: String,
    pub static_attrs: Vec<WorkspaceEdbAttr>,
}

/// Declarative, host-supplied rule projecting matching lines into open
/// [`WorkspaceEdbEdgeFact`] rows using named regex captures `src` and `dst`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceEdbEdgeRule {
    pub language: Option<String>,
    pub relation: String,
    pub pattern: String,
}

/// Prefilter and host-supplied extraction rules passed from Marlin's Scheme POO
/// search layer to narrow the workspace and project bounded open EDB facts.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WorkspaceEdbPrefilter<'a> {
    pub languages: &'a [String],
    pub documents: &'a [String],
    pub path_prefix: Option<&'a str>,
    pub extension: Option<&'a str>,
    pub required_tokens: &'a [String],
    pub entity_rules: &'a [WorkspaceEdbEntityRule],
    pub edge_rules: &'a [WorkspaceEdbEdgeRule],
    pub max_files: usize,
    pub max_facts: usize,
}

/// One indexed file fact `file(file_id, path, language)`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceEdbFileFact {
    pub file_id: u32,
    pub path: String,
    pub language: Option<String>,
}

/// One open key-value attribute fact `attr(entity_id, key, value)`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceEdbAttr {
    pub key: String,
    pub value: String,
}

/// One open structural entity fact `entity(entity_id, file_id, line, name, lang, facet_tag)`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceEdbEntityFact {
    pub entity_id: String,
    pub file_id: u32,
    pub path: String,
    pub line: usize,
    pub name: String,
    pub language: String,
    pub facet_tag: String,
    pub attrs: Vec<WorkspaceEdbAttr>,
}

/// One open structural relation fact `edge(src, relation, dst, path, line)`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceEdbEdgeFact {
    pub src: String,
    pub relation: String,
    pub dst: String,
    pub path: String,
    pub line: usize,
}

/// Generation-bound open EDB fact slice extracted from [`WorkspaceSearchIndex`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceEdbSnapshot {
    pub generation_digest: SearchGenerationDigest,
    pub indexed_files: usize,
    pub candidate_files: usize,
    pub files: Vec<WorkspaceEdbFileFact>,
    pub entities: Vec<WorkspaceEdbEntityFact>,
    pub edges: Vec<WorkspaceEdbEdgeFact>,
    pub elapsed_micros: u64,
}

struct CompiledEntityRule<'a> {
    language: Option<&'a str>,
    facet_tag: &'a str,
    regex: Regex,
    capture_names: Vec<String>,
    static_attrs: &'a [WorkspaceEdbAttr],
}

struct CompiledEdgeRule<'a> {
    language: Option<&'a str>,
    relation: &'a str,
    regex: Regex,
}

struct EdbCollector {
    max_facts: usize,
    entities: Vec<WorkspaceEdbEntityFact>,
    edges: Vec<WorkspaceEdbEdgeFact>,
}

impl EdbCollector {
    fn new(max_facts: usize) -> Self {
        Self {
            max_facts,
            entities: Vec::new(),
            edges: Vec::new(),
        }
    }

    fn is_full(&self) -> bool {
        self.entities.len() + self.edges.len() >= self.max_facts
    }

    fn apply_file(
        &mut self,
        file_id: u32,
        file: &IndexedFile,
        entity_rules: &[CompiledEntityRule<'_>],
        edge_rules: &[CompiledEdgeRule<'_>],
    ) {
        let language = file.language.unwrap_or("text");
        let matching_entity_rules: Vec<&CompiledEntityRule<'_>> = entity_rules
            .iter()
            .filter(|r| r.language.is_none_or(|lang| lang == language))
            .collect();
        let matching_edge_rules: Vec<&CompiledEdgeRule<'_>> = edge_rules
            .iter()
            .filter(|r| r.language.is_none_or(|lang| lang == language))
            .collect();

        if matching_entity_rules.is_empty() && matching_edge_rules.is_empty() {
            return;
        }

        for (idx, raw_line) in file.body.lines().enumerate() {
            if self.is_full() {
                return;
            }
            let trimmed = raw_line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let line = idx + 1;
            self.match_entity_rules(
                file_id,
                &file.relative_path,
                language,
                line,
                trimmed,
                &matching_entity_rules,
            );
            self.match_edge_rules(&file.relative_path, line, trimmed, &matching_edge_rules);
        }
    }

    fn match_entity_rules(
        &mut self,
        file_id: u32,
        path: &str,
        language: &str,
        line: usize,
        trimmed: &str,
        rules: &[&CompiledEntityRule<'_>],
    ) {
        for rule in rules {
            if self.is_full() {
                return;
            }
            let Some(caps) = rule.regex.captures(trimmed) else {
                continue;
            };
            let name = caps
                .name("name")
                .or_else(|| caps.get(1))
                .map(|m| m.as_str().trim().to_owned())
                .unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            let mut attrs = rule.static_attrs.to_vec();
            for cap_name in &rule.capture_names {
                if cap_name == "name" {
                    continue;
                }
                if let Some(m) = caps.name(cap_name) {
                    let val = m.as_str().trim();
                    if !val.is_empty() {
                        attrs.push(WorkspaceEdbAttr {
                            key: cap_name.clone(),
                            value: val.to_owned(),
                        });
                    }
                }
            }
            self.entities.push(WorkspaceEdbEntityFact {
                entity_id: format!("{language}://{path}#L{line}:{}:{name}", rule.facet_tag),
                file_id,
                path: path.to_owned(),
                line,
                name,
                language: language.to_owned(),
                facet_tag: rule.facet_tag.to_owned(),
                attrs,
            });
        }
    }

    fn match_edge_rules(
        &mut self,
        path: &str,
        line: usize,
        trimmed: &str,
        rules: &[&CompiledEdgeRule<'_>],
    ) {
        for rule in rules {
            if self.is_full() {
                return;
            }
            let Some(caps) = rule.regex.captures(trimmed) else {
                continue;
            };
            let src = caps
                .name("src")
                .or_else(|| caps.get(1))
                .map(|m| m.as_str().trim())
                .unwrap_or(path);
            let dst = caps
                .name("dst")
                .or_else(|| caps.get(2))
                .map(|m| m.as_str().trim())
                .unwrap_or("");
            if src.is_empty() || dst.is_empty() {
                continue;
            }
            self.edges.push(WorkspaceEdbEdgeFact {
                src: src.to_owned(),
                relation: rule.relation.to_owned(),
                dst: dst.to_owned(),
                path: path.to_owned(),
                line,
            });
        }
    }
}

impl WorkspaceSearchIndex {
    /// Extract bounded open EDB facts (`file`, `entity`, `attr`, `edge`) for files
    /// surviving sub-millisecond trigram, language, and topology pre-filtering using
    /// host-supplied declarative extraction rules.
    #[must_use]
    pub fn extract_prefiltered_edb(
        &self,
        prefilter: &WorkspaceEdbPrefilter<'_>,
    ) -> WorkspaceEdbSnapshot {
        let start = Instant::now();
        let max_files = if prefilter.max_files == 0 {
            DEFAULT_MAX_EDB_FILES
        } else {
            prefilter.max_files
        };
        let max_facts = if prefilter.max_facts == 0 {
            DEFAULT_MAX_EDB_FACTS
        } else {
            prefilter.max_facts
        };

        let filters = CompiledFilters::from_parts(
            prefilter.languages,
            prefilter.documents,
            prefilter.path_prefix,
            prefilter.extension,
        );
        let candidate_ids = self.select_edb_candidate_files(prefilter, &filters, max_files);
        let candidate_files = candidate_ids.len();

        let entity_rules = compile_entity_rules(prefilter.entity_rules);
        let edge_rules = compile_edge_rules(prefilter.edge_rules);
        let mut collector = EdbCollector::new(max_facts);
        let mut files = Vec::with_capacity(candidate_files);

        for file_id in candidate_ids {
            let Some(file) = self.files.get(file_id as usize) else {
                continue;
            };
            files.push(WorkspaceEdbFileFact {
                file_id,
                path: file.relative_path.clone(),
                language: file.language.map(str::to_owned),
            });
            if !collector.is_full() {
                collector.apply_file(file_id, file, &entity_rules, &edge_rules);
            }
        }

        WorkspaceEdbSnapshot {
            generation_digest: self.generation_digest.clone(),
            indexed_files: self.live_file_count,
            candidate_files,
            files,
            entities: collector.entities,
            edges: collector.edges,
            elapsed_micros: start.elapsed().as_micros() as u64,
        }
    }

    fn select_edb_candidate_files(
        &self,
        prefilter: &WorkspaceEdbPrefilter<'_>,
        filters: &CompiledFilters,
        max_files: usize,
    ) -> Vec<u32> {
        let lower_tokens: Vec<String> = prefilter
            .required_tokens
            .iter()
            .map(|t| t.trim().to_ascii_lowercase())
            .filter(|t| !t.is_empty())
            .collect();

        let trigram_terms: Vec<TrigramQuery> = lower_tokens
            .iter()
            .filter(|t| t.len() >= 3)
            .map(|t| TrigramQuery::Literal(t.as_bytes().to_vec()))
            .collect();

        let seed_ids: Vec<u32> = if !trigram_terms.is_empty() {
            let query = if trigram_terms.len() == 1 {
                trigram_terms
                    .into_iter()
                    .next()
                    .unwrap_or(TrigramQuery::All)
            } else {
                TrigramQuery::And(trigram_terms)
            };
            self.trigrams.evaluate_query(&query, &self.tombstones)
        } else {
            self.live_files().map(|(id, _)| id).collect()
        };

        let mut selected = Vec::with_capacity(seed_ids.len().min(max_files));
        for file_id in seed_ids {
            if selected.len() >= max_files {
                break;
            }
            let Some(file) = self.files.get(file_id as usize) else {
                continue;
            };
            if self
                .tombstones
                .get(file_id as usize)
                .copied()
                .unwrap_or(true)
                || !filters.matches(file)
            {
                continue;
            }
            if !lower_tokens.is_empty()
                && !lower_tokens
                    .iter()
                    .all(|tok| memchr::memmem::find(&file.body_lower, tok.as_bytes()).is_some())
            {
                continue;
            }
            selected.push(file_id);
        }
        selected
    }
}

fn compile_entity_rules(rules: &[WorkspaceEdbEntityRule]) -> Vec<CompiledEntityRule<'_>> {
    rules
        .iter()
        .filter_map(|rule| {
            let regex = Regex::new(&rule.pattern).ok()?;
            let capture_names = regex.capture_names().flatten().map(str::to_owned).collect();
            Some(CompiledEntityRule {
                language: rule.language.as_deref(),
                facet_tag: &rule.facet_tag,
                regex,
                capture_names,
                static_attrs: &rule.static_attrs,
            })
        })
        .collect()
}

fn compile_edge_rules(rules: &[WorkspaceEdbEdgeRule]) -> Vec<CompiledEdgeRule<'_>> {
    rules
        .iter()
        .filter_map(|rule| {
            let regex = Regex::new(&rule.pattern).ok()?;
            Some(CompiledEdgeRule {
                language: rule.language.as_deref(),
                relation: &rule.relation,
                regex,
            })
        })
        .collect()
}

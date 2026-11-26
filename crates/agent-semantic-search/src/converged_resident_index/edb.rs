// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Zero-regex, protocol-native Open EDB (Extensional Database) fact store and
//! projection layer for [`WorkspaceSearchIndex`].
//!
//! ASP's Rust search engine never runs ad hoc regex matchers or language mini-parsers:
//! - Heavy language providers (`asp-rust`, `asp-typescript`, `asp-python`, `asp-julia`,
//!   `asp-gerbil-scheme` over HTTP/gRPC) emit native compiler/parser AST facts using
//!   `schemas/semantic-open-edb.v1.schema.json` (`kind` + native `language_kind` + `source`)
//!   and ingest them via [`WorkspaceSearchIndex::ingest_open_edb_facts`].
//! - Lightweight/DSL/query languages parsed in Marlin's Scheme plane (`:gerbil-parser`
//!   or `orgize-fold`) ingest into the same universal Open EDB protocol.
//! - Files without ingested provider facts yet fall back deterministically to the
//!   resident index's pre-indexed symbols/imports (`source = "resident-index"`).
//! - File edits or deletions via [`WorkspaceSearchIndex::apply_file_delta`] automatically
//!   invalidate stale per-file provider facts on the generation boundary.

use std::time::Instant;

use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

use crate::SearchGenerationDigest;
use crate::trigram_query::TrigramQuery;

use super::index::{IndexedFile, WorkspaceContextLine, WorkspaceSearchIndex};
use super::provider_ingest::{
    parse_native_syntax_fact_index_json, parse_provider_projection_batch_json,
};
use super::query::{CompiledFilters, extract_line_slice};
use super::symbols::WorkspaceSymbolKind;

/// Canonical JSON Schema identifier for converged Open EDB provider fact batches.
pub const SEMANTIC_OPEN_EDB_SCHEMA_ID: &str = "agent.semantic-protocols.semantic-open-edb";
/// Canonical JSON Schema version for converged Open EDB provider fact batches.
pub const SEMANTIC_OPEN_EDB_SCHEMA_VERSION: &str = "1";

const DEFAULT_MAX_EDB_FILES: usize = 256;
const DEFAULT_MAX_EDB_FACTS: usize = 4096;

/// Sub-millisecond trigram and topology prefilter passed from Marlin's Scheme
/// `:gerbil-ascent` layer to project a bounded Open EDB slice (zero regex rules).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WorkspaceEdbPrefilter<'a> {
    pub languages: &'a [String],
    pub documents: &'a [String],
    pub path_prefix: Option<&'a str>,
    pub extension: Option<&'a str>,
    pub required_tokens: &'a [String],
    pub max_files: usize,
    pub max_facts: usize,
}

/// One indexed file fact `file(file_id, lang, path)`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEdbFileFact {
    pub file_id: u32,
    pub path: String,
    pub language: Option<String>,
}

/// One extensible key-value attribute fact `attr(entity_id, key, value)`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEdbAttr {
    pub key: String,
    pub value: String,
}

/// One universal structural entity fact aligned 1:1 with ASP's dual-layer
/// ontology (`kind` + native parser `language_kind` + `source`):
/// `entity(entity_id, file_id, language, kind, language_kind, name, line)`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEdbEntityFact {
    #[serde(default)]
    pub entity_id: String,
    #[serde(default)]
    pub file_id: u32,
    pub path: String,
    pub line: usize,
    pub name: String,
    #[serde(default)]
    pub language: String,
    pub kind: String,
    #[serde(default)]
    pub language_kind: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub attrs: Vec<WorkspaceEdbAttr>,
}

/// One universal structural relation fact `edge(src, dst, relation, path, line)`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEdbEdgeFact {
    pub src: String,
    pub relation: String,
    pub dst: String,
    pub path: String,
    #[serde(default)]
    pub line: usize,
}

/// Protocol-native ingestion batch conforming to `schemas/semantic-open-edb.v1.schema.json`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceOpenEdbIngestBatch {
    #[serde(default)]
    pub schema_id: Option<String>,
    #[serde(default)]
    pub schema_version: Option<String>,
    #[serde(default)]
    pub language_id: Option<String>,
    #[serde(default)]
    pub provider_id: Option<String>,
    #[serde(default)]
    pub entities: Vec<WorkspaceEdbEntityFact>,
    #[serde(default)]
    pub edges: Vec<WorkspaceEdbEdgeFact>,
}

impl WorkspaceOpenEdbIngestBatch {
    /// Convert an ASP `semantic-native-syntax-fact-index.v1.schema.json` payload
    /// directly into a converged Open EDB ingestion batch (zero regex).
    pub fn from_native_syntax_fact_index_json(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        parse_native_syntax_fact_index_json(bytes)
    }

    /// Convert an ASP `provider-language-projection-batch-response` payload
    /// directly into a converged Open EDB ingestion batch (zero regex).
    pub fn from_provider_projection_batch_json(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        parse_provider_projection_batch_json(bytes)
    }
}

/// One live workspace file awaiting native-parser or `:gerbil-parser` Open EDB hydration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceOpenEdbPendingFile {
    pub path: String,
    pub language: String,
    pub body: String,
}

/// Deterministic receipt returned by [`WorkspaceSearchIndex::ingest_open_edb_facts`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceOpenEdbIngestReceipt {
    pub generation_digest: SearchGenerationDigest,
    pub updated_files: usize,
    pub ingested_entities: usize,
    pub ingested_edges: usize,
    pub skipped_facts: usize,
}

/// Generation-bound Open EDB fact slice projected from [`WorkspaceSearchIndex`].
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

fn map_symbol_kind_to_ontology(kind: WorkspaceSymbolKind) -> (&'static str, &'static str) {
    match kind {
        WorkspaceSymbolKind::Function => ("function", "fn"),
        WorkspaceSymbolKind::Type => ("type", "type"),
        WorkspaceSymbolKind::Trait => ("trait", "trait"),
        WorkspaceSymbolKind::Module => ("module", "mod"),
        WorkspaceSymbolKind::Constant => ("constant", "const"),
        WorkspaceSymbolKind::Macro => ("macro", "macro"),
        WorkspaceSymbolKind::Class => ("class", "class"),
        WorkspaceSymbolKind::Interface => ("interface", "interface"),
        WorkspaceSymbolKind::Heading => ("heading", "heading"),
    }
}

struct EdbProjector {
    max_facts: usize,
    entities: Vec<WorkspaceEdbEntityFact>,
    edges: Vec<WorkspaceEdbEdgeFact>,
}

impl EdbProjector {
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

    fn project_file(&mut self, file_id: u32, file: &IndexedFile) {
        if !file.open_edb_entities.is_empty() || !file.open_edb_edges.is_empty() {
            for entity in &file.open_edb_entities {
                if self.is_full() {
                    return;
                }
                let mut row = entity.clone();
                row.file_id = file_id;
                self.entities.push(row);
            }
            for edge in &file.open_edb_edges {
                if self.is_full() {
                    return;
                }
                self.edges.push(edge.clone());
            }
            return;
        }

        let language = file.language.unwrap_or("text");
        for sym in &file.symbols {
            if self.is_full() {
                return;
            }
            let (kind, language_kind) = map_symbol_kind_to_ontology(sym.kind);
            self.entities.push(WorkspaceEdbEntityFact {
                entity_id: format!(
                    "{language}://{}#L{}:{}:{}",
                    file.relative_path, sym.line_number, kind, sym.name
                ),
                file_id,
                path: file.relative_path.clone(),
                line: sym.line_number,
                name: sym.name.clone(),
                language: language.to_owned(),
                kind: kind.to_owned(),
                language_kind: language_kind.to_owned(),
                source: "resident-index".to_owned(),
                attrs: Vec::new(),
            });
        }
        for imp in &file.imports {
            if self.is_full() {
                return;
            }
            self.edges.push(WorkspaceEdbEdgeFact {
                src: file.relative_path.clone(),
                relation: "imports".to_owned(),
                dst: imp.clone(),
                path: file.relative_path.clone(),
                line: 1,
            });
        }
    }
}

impl WorkspaceSearchIndex {
    /// Return surrounding RAM source lines for a live workspace file at
    /// 1-based `focus_line` (used by Helm split-pane preview for `:gerbil-ascent`
    /// and Dusa query answer rows with zero disk I/O).
    #[must_use]
    pub fn live_file_context(
        &self,
        relative_path: &str,
        focus_line: usize,
        radius: usize,
    ) -> Vec<WorkspaceContextLine> {
        let clean = relative_path.replace('\\', "/");
        let Some(&file_id) = self.path_to_id.get(&clean) else {
            return Vec::new();
        };
        if self.is_tombstoned(file_id) {
            return Vec::new();
        }
        let Some(file) = self.files.get(file_id as usize) else {
            return Vec::new();
        };
        let total_lines = file.line_offsets.len().max(1);
        let focus = focus_line.clamp(1, total_lines);
        let start = focus.saturating_sub(radius).max(1);
        let end = focus.saturating_add(radius).min(total_lines);
        let mut out = Vec::with_capacity(end.saturating_sub(start) + 1);
        for line_number in start..=end {
            out.push(WorkspaceContextLine {
                line_number,
                text: extract_line_slice(file, line_number).to_owned(),
                is_match: line_number == focus,
            });
        }
        out
    }

    /// Return live workspace files matching `languages` that do not yet have
    /// ingested Open EDB facts attached (for incremental native/Scheme hydration).
    #[must_use]
    pub fn live_files_needing_open_edb(
        &self,
        languages: &[&str],
        max_files: usize,
    ) -> Vec<WorkspaceOpenEdbPendingFile> {
        let limit = if max_files == 0 {
            usize::MAX
        } else {
            max_files
        };
        let mut out = Vec::new();
        for (_, file) in self.live_files() {
            if out.len() >= limit {
                break;
            }
            if !file.open_edb_entities.is_empty() || !file.open_edb_edges.is_empty() {
                continue;
            }
            let Some(lang) = file.language else {
                continue;
            };
            if !languages.is_empty() && !languages.contains(&lang) {
                continue;
            }
            out.push(WorkspaceOpenEdbPendingFile {
                path: file.relative_path.clone(),
                language: lang.to_owned(),
                body: file.body.clone(),
            });
        }
        out
    }

    /// Ingest protocol-native Open EDB facts emitted by external language providers
    /// (`asp-rust`, `asp-typescript`, `asp-python`, `asp-julia`, `asp-gerbil-scheme`)
    /// or Marlin's Scheme `:gerbil-parser` plane, replacing cached per-file facts for
    /// each referenced live workspace file.
    pub fn ingest_open_edb_facts(
        &mut self,
        batch: WorkspaceOpenEdbIngestBatch,
    ) -> WorkspaceOpenEdbIngestReceipt {
        let mut by_path: FxHashMap<
            String,
            (Vec<WorkspaceEdbEntityFact>, Vec<WorkspaceEdbEdgeFact>),
        > = FxHashMap::default();
        let batch_lang = batch.language_id.as_deref();

        for mut entity in batch.entities {
            let clean_path = entity.path.replace('\\', "/");
            entity.path = clean_path.clone();
            by_path.entry(clean_path).or_default().0.push(entity);
        }
        for mut edge in batch.edges {
            let clean_path = edge.path.replace('\\', "/");
            edge.path = clean_path.clone();
            by_path.entry(clean_path).or_default().1.push(edge);
        }

        let mut updated_files = 0usize;
        let mut ingested_entities = 0usize;
        let mut ingested_edges = 0usize;
        let mut skipped_facts = 0usize;

        for (path, (mut entities, edges)) in by_path {
            let Some(&file_id) = self.path_to_id.get(&path) else {
                skipped_facts += entities.len() + edges.len();
                continue;
            };
            if self.is_tombstoned(file_id) {
                skipped_facts += entities.len() + edges.len();
                continue;
            }
            let Some(file) = self.files.get_mut(file_id as usize) else {
                skipped_facts += entities.len() + edges.len();
                continue;
            };

            let fallback_lang = batch_lang.or(file.language).unwrap_or("text");
            for entity in &mut entities {
                entity.file_id = file_id;
                if entity.language.is_empty() {
                    entity.language = fallback_lang.to_owned();
                }
                if entity.language_kind.is_empty() {
                    entity.language_kind = entity.kind.clone();
                }
                if entity.source.is_empty() {
                    entity.source = "native-parser".to_owned();
                }
                if entity.entity_id.is_empty() {
                    entity.entity_id = format!(
                        "{}://{}#L{}:{}:{}",
                        entity.language,
                        entity.path,
                        entity.line,
                        entity.language_kind,
                        entity.name
                    );
                }
            }

            ingested_entities += entities.len();
            ingested_edges += edges.len();
            file.open_edb_entities = entities;
            file.open_edb_edges = edges;
            updated_files += 1;
        }

        WorkspaceOpenEdbIngestReceipt {
            generation_digest: self.generation_digest.clone(),
            updated_files,
            ingested_entities,
            ingested_edges,
            skipped_facts,
        }
    }

    /// Project bounded Open EDB facts (`file`, `entity`, `edge`, `attr`) for files
    /// surviving sub-millisecond trigram, language, and topology prefiltering.
    ///
    /// Streams ingested native-parser / `:gerbil-parser` facts when attached to a file
    /// and otherwise projects the file's zero-regex resident symbols/imports.
    #[must_use]
    pub fn project_prefiltered_edb(
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

        let mut projector = EdbProjector::new(max_facts);
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
            if !projector.is_full() {
                projector.project_file(file_id, file);
            }
        }

        WorkspaceEdbSnapshot {
            generation_digest: self.generation_digest.clone(),
            indexed_files: self.live_file_count,
            candidate_files,
            files,
            entities: projector.entities,
            edges: projector.edges,
            elapsed_micros: u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX),
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
            if self.is_tombstoned(file_id) || !filters.matches(file) {
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

// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Generation-local Tantivy lexical authority.
//!
//! The index is built once from one immutable owner generation. Queries return
//! owner identities only; executable selectors and bytes remain generation-owned.

use std::path::Path;

use tantivy::Index;
use tantivy::IndexReader;
use tantivy::IndexSettings;
use tantivy::Order;
use tantivy::TantivyDocument;
use tantivy::Term;
use tantivy::collector::TopDocs;
use tantivy::directory::MmapDirectory;
use tantivy::doc;
use tantivy::indexer::NoMergePolicy;
use tantivy::indexer::SingleSegmentIndexWriter;
use tantivy::query::AllQuery;
use tantivy::query::BooleanQuery;
use tantivy::query::Query;
use tantivy::query::QueryParser;
use tantivy::query::TermQuery;
use tantivy::schema::FAST;
use tantivy::schema::Field;
use tantivy::schema::IndexRecordOption;
use tantivy::schema::Schema;
use tantivy::schema::TEXT;
use tantivy::schema::TextFieldIndexing;
use tantivy::schema::TextOptions;

pub(crate) struct TantivyLexicalDocument {
    pub(crate) exact_terms: Vec<String>,
    pub(crate) title: String,
    pub(crate) body: String,
}

pub(crate) struct TantivyLexicalIndex {
    index: Index,
    reader: IndexReader,
    term_field: Field,
    title_field: Field,
    body_field: Field,
    owner_count: usize,
}

impl std::fmt::Debug for TantivyLexicalIndex {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TantivyLexicalIndex")
            .field("owner_count", &self.owner_count)
            .finish_non_exhaustive()
    }
}

impl TantivyLexicalIndex {
    pub(crate) fn build_with_resources(
        documents: &[TantivyLexicalDocument],
        resources: crate::ResidentIndexBuildResources,
    ) -> Result<Self, String> {
        let (schema, term_field, title_field, body_field, owner_id_field) = lexical_schema();
        let index = Index::create_in_ram(schema);
        build_index(
            index,
            term_field,
            title_field,
            body_field,
            owner_id_field,
            documents,
            resources,
        )
    }

    pub(crate) fn build_in_directory(
        directory: &Path,
        documents: &[TantivyLexicalDocument],
        resources: crate::ResidentIndexBuildResources,
    ) -> Result<(Self, String), String> {
        std::fs::create_dir_all(directory)
            .map_err(|error| format!("create Tantivy generation directory: {error}"))?;
        let (schema, term_field, title_field, body_field, owner_id_field) = lexical_schema();
        let mmap = MmapDirectory::open(directory)
            .map_err(|error| format!("open Tantivy generation directory: {error}"))?;
        let index = Index::create(mmap, schema, IndexSettings::default())
            .map_err(|error| format!("create Tantivy generation artifact: {error}"))?;
        let lexical = build_index(
            index,
            term_field,
            title_field,
            body_field,
            owner_id_field,
            documents,
            resources,
        )?;
        let digest = tantivy_directory_digest(directory)?;
        Ok((lexical, digest))
    }

    pub(crate) fn open_from_directory(
        directory: &Path,
        expected_owner_count: usize,
        expected_artifact_digest: &str,
    ) -> Result<Self, String> {
        let actual_artifact_digest = tantivy_directory_digest(directory)?;
        if actual_artifact_digest != expected_artifact_digest {
            return Err("Tantivy generation artifact digest mismatch".to_owned());
        }
        let index = Index::open_in_dir(directory)
            .map_err(|error| format!("open Tantivy generation artifact: {error}"))?;
        let schema = index.schema();
        let term_field = schema
            .get_field("term")
            .map_err(|error| format!("Tantivy generation term field is absent: {error}"))?;
        let title_field = schema
            .get_field("title")
            .map_err(|error| format!("Tantivy generation title field is absent: {error}"))?;
        let body_field = schema
            .get_field("body")
            .map_err(|error| format!("Tantivy generation body field is absent: {error}"))?;
        schema
            .get_field("ownerId")
            .map_err(|error| format!("Tantivy generation ownerId field is absent: {error}"))?;
        let reader = index
            .reader()
            .map_err(|error| format!("open resident Tantivy reader: {error}"))?;
        let actual_owner_count = usize::try_from(reader.searcher().num_docs())
            .map_err(|_| "Tantivy generation owner count overflow".to_owned())?;
        if actual_owner_count != expected_owner_count {
            return Err("Tantivy generation owner coverage mismatch".to_owned());
        }
        Ok(Self {
            index,
            reader,
            term_field,
            title_field,
            body_field,
            owner_count: expected_owner_count,
        })
    }

    pub(crate) fn artifact_digest(directory: &Path) -> Result<String, String> {
        tantivy_directory_digest(directory)
    }

    pub(crate) fn search(&self, terms: &[String], limit: usize) -> Result<Vec<usize>, String> {
        let query: Box<dyn Query> = if terms.is_empty() {
            Box::new(AllQuery)
        } else {
            Box::new(BooleanQuery::intersection(
                terms
                    .iter()
                    .map(|term| {
                        Box::new(TermQuery::new(
                            Term::from_field_text(self.term_field, term),
                            IndexRecordOption::Basic,
                        )) as Box<dyn Query>
                    })
                    .collect(),
            ))
        };
        let searcher = self.reader.searcher();
        let top_docs = searcher
            .search(
                &query,
                &TopDocs::with_limit(limit.min(self.owner_count).max(1))
                    .order_by_fast_field::<u64>("ownerId", Order::Asc),
            )
            .map_err(|error| format!("query resident Tantivy generation: {error}"))?;
        top_docs
            .into_iter()
            .map(|(owner_id, _)| {
                owner_id
                    .and_then(|value| usize::try_from(value).ok())
                    .ok_or_else(|| "resident Tantivy ownerId is missing".to_owned())
            })
            .collect()
    }

    pub(crate) fn search_expression(
        &self,
        expression: &str,
        required_terms: &[String],
        limit: usize,
    ) -> Result<Vec<usize>, String> {
        let mut parser =
            QueryParser::for_index(&self.index, vec![self.title_field, self.body_field]);
        parser.set_field_boost(self.title_field, 2.0);
        let parsed = parser
            .parse_query(expression)
            .map_err(|error| format!("parse native Tantivy expression: {error}"))?;
        let query: Box<dyn Query> = if required_terms.is_empty() {
            parsed
        } else {
            let mut clauses = Vec::<Box<dyn Query>>::with_capacity(required_terms.len() + 1);
            clauses.push(parsed);
            clauses.extend(required_terms.iter().map(|term| {
                Box::new(TermQuery::new(
                    Term::from_field_text(self.term_field, term),
                    IndexRecordOption::Basic,
                )) as Box<dyn Query>
            }));
            Box::new(BooleanQuery::intersection(clauses))
        };
        let searcher = self.reader.searcher();
        let matches = searcher
            .search(
                &query,
                &TopDocs::with_limit(limit.min(self.owner_count).max(1)).order_by_score(),
            )
            .map_err(|error| format!("query resident Tantivy expression: {error}"))?;
        matches
            .into_iter()
            .map(|(_, address)| {
                searcher
                    .segment_reader(address.segment_ord)
                    .fast_fields()
                    .u64("ownerId")
                    .map_err(|error| format!("read resident Tantivy ownerId: {error}"))?
                    .first(address.doc_id)
                    .and_then(|owner_id| usize::try_from(owner_id).ok())
                    .ok_or_else(|| "resident Tantivy ownerId is missing".to_owned())
            })
            .collect()
    }
}

fn lexical_schema() -> (Schema, Field, Field, Field, Field) {
    let mut schema = Schema::builder();
    let term_options = TextOptions::default().set_indexing_options(
        TextFieldIndexing::default()
            .set_tokenizer("raw")
            .set_fieldnorms(false)
            .set_index_option(IndexRecordOption::Basic),
    );
    let term_field = schema.add_text_field("term", term_options);
    let title_field = schema.add_text_field("title", TEXT);
    let body_field = schema.add_text_field("body", TEXT);
    let owner_id_field = schema.add_u64_field("ownerId", FAST);
    (
        schema.build(),
        term_field,
        title_field,
        body_field,
        owner_id_field,
    )
}

fn build_index(
    index: Index,
    term_field: Field,
    title_field: Field,
    body_field: Field,
    owner_id_field: Field,
    documents: &[TantivyLexicalDocument],
    resources: crate::ResidentIndexBuildResources,
) -> Result<TantivyLexicalIndex, String> {
    let index = match resources.strategy() {
        crate::ResidentIndexBuildStrategy::SingleSegmentBulk => {
            let mut writer = SingleSegmentIndexWriter::<TantivyDocument>::new(
                index,
                resources.memory_budget_bytes(),
            )
            .map_err(|error| format!("create resident Tantivy single-segment writer: {error}"))?;
            for (owner_id, document) in documents.iter().enumerate() {
                writer
                    .add_document(owner_document(
                        term_field,
                        title_field,
                        body_field,
                        owner_id_field,
                        owner_id,
                        document,
                    ))
                    .map_err(|error| format!("add resident Tantivy owner: {error}"))?;
            }
            writer
                .finalize()
                .map_err(|error| format!("finalize resident Tantivy single segment: {error}"))?
        }
        crate::ResidentIndexBuildStrategy::ParallelSegments => {
            let mut writer = index
                .writer_with_num_threads(
                    resources.indexing_threads(),
                    resources.memory_budget_bytes(),
                )
                .map_err(|error| format!("create resident Tantivy writer: {error}"))?;
            // One resident attachment is immutable after publication. Merging
            // its freshly-built segments cannot improve update behavior and
            // only serializes publication behind a second complete rewrite.
            // Tantivy searches all committed segments directly, so retain the
            // parallel build output and let a later content generation build
            // its own independent attachment.
            writer.set_merge_policy(Box::new(NoMergePolicy));
            for (owner_id, document) in documents.iter().enumerate() {
                writer
                    .add_document(owner_document(
                        term_field,
                        title_field,
                        body_field,
                        owner_id_field,
                        owner_id,
                        document,
                    ))
                    .map_err(|error| format!("add resident Tantivy owner: {error}"))?;
            }
            writer
                .commit()
                .map_err(|error| format!("commit resident Tantivy generation: {error}"))?;
            writer
                .wait_merging_threads()
                .map_err(|error| format!("finish resident Tantivy generation merges: {error}"))?;
            index
        }
    };
    let reader = index
        .reader()
        .map_err(|error| format!("open resident Tantivy reader: {error}"))?;
    Ok(TantivyLexicalIndex {
        index,
        reader,
        term_field,
        title_field,
        body_field,
        owner_count: documents.len(),
    })
}

fn owner_document(
    term_field: Field,
    title_field: Field,
    body_field: Field,
    owner_id_field: Field,
    owner_id: usize,
    source: &TantivyLexicalDocument,
) -> TantivyDocument {
    let mut document = doc!(owner_id_field => owner_id as u64);
    for term in &source.exact_terms {
        document.add_text(term_field, term);
    }
    document.add_text(title_field, &source.title);
    document.add_text(body_field, &source.body);
    document
}

fn tantivy_directory_digest(directory: &Path) -> Result<String, String> {
    let mut files = std::fs::read_dir(directory)
        .map_err(|error| format!("read Tantivy generation directory: {error}"))?
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .map_err(|error| format!("read Tantivy generation entry: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    files.sort();
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"agent.semantic-protocols.tantivy-lexical-artifact.v1\0");
    for path in files {
        if !path.is_file() {
            return Err("Tantivy generation artifact contains a non-file entry".to_owned());
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| "Tantivy generation filename is not UTF-8".to_owned())?;
        let bytes = std::fs::read(&path)
            .map_err(|error| format!("read Tantivy generation artifact file: {error}"))?;
        hasher.update(&(name.len() as u64).to_le_bytes());
        hasher.update(name.as_bytes());
        hasher.update(&(bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
    }
    Ok(format!("blake3-256:{}", hasher.finalize().to_hex()))
}

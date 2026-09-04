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
use tantivy::query::TermQuery;
use tantivy::schema::FAST;
use tantivy::schema::Field;
use tantivy::schema::IndexRecordOption;
use tantivy::schema::Schema;
use tantivy::schema::TextFieldIndexing;
use tantivy::schema::TextOptions;

pub(crate) struct TantivyLexicalIndex {
    _index: Index,
    reader: IndexReader,
    term_field: Field,
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
        owner_terms: &[Vec<String>],
        resources: crate::ResidentIndexBuildResources,
    ) -> Result<Self, String> {
        let (schema, term_field, owner_id_field) = lexical_schema();
        let index = Index::create_in_ram(schema);
        build_index(index, term_field, owner_id_field, owner_terms, resources)
    }

    pub(crate) fn build_in_directory(
        directory: &Path,
        owner_terms: &[Vec<String>],
        resources: crate::ResidentIndexBuildResources,
    ) -> Result<(Self, String), String> {
        std::fs::create_dir_all(directory)
            .map_err(|error| format!("create Tantivy generation directory: {error}"))?;
        let (schema, term_field, owner_id_field) = lexical_schema();
        let mmap = MmapDirectory::open(directory)
            .map_err(|error| format!("open Tantivy generation directory: {error}"))?;
        let index = Index::create(mmap, schema, IndexSettings::default())
            .map_err(|error| format!("create Tantivy generation artifact: {error}"))?;
        let lexical = build_index(index, term_field, owner_id_field, owner_terms, resources)?;
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
            _index: index,
            reader,
            term_field,
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
}

fn lexical_schema() -> (Schema, Field, Field) {
    let mut schema = Schema::builder();
    let term_options = TextOptions::default().set_indexing_options(
        TextFieldIndexing::default()
            .set_tokenizer("raw")
            .set_fieldnorms(false)
            .set_index_option(IndexRecordOption::Basic),
    );
    let term_field = schema.add_text_field("term", term_options);
    let owner_id_field = schema.add_u64_field("ownerId", FAST);
    (schema.build(), term_field, owner_id_field)
}

fn build_index(
    index: Index,
    term_field: Field,
    owner_id_field: Field,
    owner_terms: &[Vec<String>],
    resources: crate::ResidentIndexBuildResources,
) -> Result<TantivyLexicalIndex, String> {
    let index = match resources.strategy() {
        crate::ResidentIndexBuildStrategy::SingleSegmentBulk => {
            let mut writer = SingleSegmentIndexWriter::<TantivyDocument>::new(
                index,
                resources.memory_budget_bytes(),
            )
            .map_err(|error| format!("create resident Tantivy single-segment writer: {error}"))?;
            for (owner_id, terms) in owner_terms.iter().enumerate() {
                writer
                    .add_document(owner_document(term_field, owner_id_field, owner_id, terms))
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
            for (owner_id, terms) in owner_terms.iter().enumerate() {
                writer
                    .add_document(owner_document(term_field, owner_id_field, owner_id, terms))
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
        _index: index,
        reader,
        term_field,
        owner_count: owner_terms.len(),
    })
}

fn owner_document(
    term_field: Field,
    owner_id_field: Field,
    owner_id: usize,
    terms: &[String],
) -> TantivyDocument {
    let mut document = doc!(owner_id_field => owner_id as u64);
    for term in terms {
        document.add_text(term_field, term);
    }
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

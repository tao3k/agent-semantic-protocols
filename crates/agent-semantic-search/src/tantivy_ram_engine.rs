// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! In-RAM Tantivy BM25 + Levenshtein `FuzzyTermQuery` lexical engine for sub-ms
//! interactive and agentic search.
//!
//! Built once during workspace warm-up via `tantivy::Index::create_in_ram` with
//! code-aware token normalization (splitting path segments, snake_case, and
//! camelCase transitions while preserving identifier stems).

use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, FuzzyTermQuery, Occur, Query, QueryParser};
use tantivy::schema::{FAST, Field, STORED, Schema, TEXT, Value};
use tantivy::{Index, IndexReader, ReloadPolicy, TantivyDocument, Term, doc};

const TANTIVY_WRITER_HEAP_BYTES: usize = 15 * 1024 * 1024;
const MAX_INDEXED_BODY_TOKENS_BYTES: usize = 256 * 1024;

#[derive(Clone, Copy, Debug)]
pub(crate) struct TantivyRamHit {
    pub(crate) file_id: u32,
    pub(crate) bm25_score: f32,
    pub(crate) is_fuzzy: bool,
}

pub(crate) struct TantivyRamEngine {
    index: Index,
    reader: IndexReader,
    file_id_field: Field,
    title_field: Field,
    body_field: Field,
}

impl TantivyRamEngine {
    pub(crate) fn build(docs: &[(u32, &str, &str)]) -> Option<Self> {
        let mut schema_builder = Schema::builder();
        let file_id_field = schema_builder.add_u64_field("file_id", STORED | FAST);
        let title_field = schema_builder.add_text_field("title", TEXT);
        let body_field = schema_builder.add_text_field("body", TEXT);
        let schema = schema_builder.build();

        let index = Index::create_in_ram(schema);
        let mut writer = index
            .writer_with_num_threads(1, TANTIVY_WRITER_HEAP_BYTES)
            .ok()?;

        for &(file_id, path, body) in docs {
            let title_tokens = normalize_code_tokens(path, 2048);
            let body_tokens = normalize_code_tokens(body, MAX_INDEXED_BODY_TOKENS_BYTES);
            let _ = writer.add_document(doc!(
                file_id_field => u64::from(file_id),
                title_field => title_tokens,
                body_field => body_tokens,
            ));
        }
        writer.commit().ok()?;
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
            .ok()?;
        let _ = reader.reload();

        Some(Self {
            index,
            reader,
            file_id_field,
            title_field,
            body_field,
        })
    }

    pub(crate) fn search(
        &self,
        raw_query: &str,
        limit: usize,
        fuzzy_distance: u8,
    ) -> Vec<TantivyRamHit> {
        let trimmed = raw_query.trim();
        if trimmed.is_empty() || limit == 0 {
            return Vec::new();
        }
        let searcher = self.reader.searcher();
        let mut parser =
            QueryParser::for_index(&self.index, vec![self.title_field, self.body_field]);
        parser.set_field_boost(self.title_field, 2.5);
        parser.set_conjunction_by_default();

        let mut results: Vec<TantivyRamHit> = Vec::new();
        let mut seen = std::collections::HashSet::new();

        let normalized_q = normalize_query_for_parser(trimmed);
        if !normalized_q.is_empty()
            && let Ok(parsed_query) = parser.parse_query(&normalized_q)
            && let Ok(top_docs) =
                searcher.search(&parsed_query, &TopDocs::with_limit(limit).order_by_score())
        {
            for (score, addr) in top_docs {
                if let Ok(doc) = searcher.doc::<TantivyDocument>(addr)
                    && let Some(fid) = extract_file_id(&doc, self.file_id_field)
                    && seen.insert(fid)
                {
                    results.push(TantivyRamHit {
                        file_id: fid,
                        bm25_score: score,
                        is_fuzzy: false,
                    });
                }
            }
        }

        if fuzzy_distance > 0 && results.len() < limit {
            let distance = fuzzy_distance.min(2);
            let fuzzy_terms = extract_fuzzy_terms(trimmed);
            if !fuzzy_terms.is_empty() {
                let remaining = limit.saturating_sub(results.len());
                let fuzzy_query = build_fuzzy_boolean_query(
                    &fuzzy_terms,
                    self.title_field,
                    self.body_field,
                    distance,
                );
                if let Ok(top_docs) = searcher.search(
                    &fuzzy_query,
                    &TopDocs::with_limit(remaining * 2).order_by_score(),
                ) {
                    for (score, addr) in top_docs {
                        if let Ok(doc) = searcher.doc::<TantivyDocument>(addr)
                            && let Some(fid) = extract_file_id(&doc, self.file_id_field)
                            && seen.insert(fid)
                        {
                            results.push(TantivyRamHit {
                                file_id: fid,
                                bm25_score: score,
                                is_fuzzy: true,
                            });
                            if results.len() >= limit {
                                break;
                            }
                        }
                    }
                }
            }
        }

        results
    }
}

fn extract_file_id(doc: &TantivyDocument, field: Field) -> Option<u32> {
    let val = doc.get_first(field)?.as_u64()?;
    u32::try_from(val).ok()
}

fn build_fuzzy_boolean_query(
    terms: &[String],
    title_field: Field,
    body_field: Field,
    distance: u8,
) -> BooleanQuery {
    let mut clauses: Vec<(Occur, Box<dyn Query>)> = Vec::with_capacity(terms.len() * 2);
    for term in terms {
        let title_q = FuzzyTermQuery::new(Term::from_field_text(title_field, term), distance, true);
        let body_q = FuzzyTermQuery::new(Term::from_field_text(body_field, term), distance, true);
        let either = BooleanQuery::new(vec![
            (Occur::Should, Box::new(title_q)),
            (Occur::Should, Box::new(body_q)),
        ]);
        clauses.push((Occur::Must, Box::new(either)));
    }
    BooleanQuery::new(clauses)
}

fn normalize_code_tokens(text: &str, max_bytes: usize) -> String {
    let slice = if text.len() > max_bytes {
        let mut end = max_bytes;
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }
        &text[..end]
    } else {
        text
    };
    let mut out = String::with_capacity(slice.len().min(max_bytes) + 64);
    let mut prev_lower = false;
    for ch in slice.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() && prev_lower {
                out.push(' ');
            }
            out.push(ch.to_ascii_lowercase());
            prev_lower = ch.is_ascii_lowercase();
        } else {
            out.push(' ');
            prev_lower = false;
        }
    }
    out
}

fn normalize_query_for_parser(raw: &str) -> String {
    let bounded = if raw.len() > 256 {
        let mut end = 256;
        while end > 0 && !raw.is_char_boundary(end) {
            end -= 1;
        }
        &raw[..end]
    } else {
        raw
    };
    if bounded.contains("title:")
        || bounded.contains("body:")
        || bounded.contains(" OR ")
        || bounded.contains(" AND ")
    {
        return bounded.to_owned();
    }
    normalize_code_tokens(bounded, 256)
}

fn extract_fuzzy_terms(raw: &str) -> Vec<String> {
    normalize_code_tokens(raw, 256)
        .split_whitespace()
        .filter(|tok| tok.len() >= 4 && !matches!(*tok, "title" | "body" | "and" | "or" | "not"))
        .take(4)
        .map(str::to_owned)
        .collect()
}

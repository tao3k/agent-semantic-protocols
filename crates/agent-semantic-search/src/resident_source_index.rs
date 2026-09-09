// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Immutable, process-local source-index query data plane.
//!
//! Construction belongs to generation admission. Query methods perform no
//! filesystem, database, provider, socket, or scheduler work.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;

use agent_semantic_content_identity::SourceSnapshotEvidence;

use crate::source_index_lookup_terms;

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResidentSearchAuthority {
    pub language_id: agent_semantic_config::LanguageId,
    pub provider_id: agent_semantic_config::ProviderId,
}

/// Stable navigation keys admitted to the durable shallow search projection.
///
/// Source text and nested syntax items are intentionally excluded. Those facts
/// belong to owner-local dynamic projections keyed by the owner content digest.
#[must_use]
pub fn resident_navigation_keys(owner_path: &str) -> Vec<String> {
    source_index_lookup_terms(owner_path)
}

const RESIDENT_LEXICAL_COVERAGE_KEY_LIMIT: usize = 4_096;
const RESIDENT_LEXICAL_TOKEN_BYTES_LIMIT: usize = 128;

pub struct ResidentLexicalCoverageInput<'a> {
    pub owner_path: &'a str,
    pub source: &'a [u8],
    pub parser_query_keys: Vec<String>,
}

/// CPU and arena authority supplied by the resident ASP Server generation
/// builder. Search does not infer machine capacity or hide a fallback budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResidentIndexBuildStrategy {
    SingleSegmentBulk,
    ParallelSegments,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResidentIndexBuildResources {
    indexing_threads: usize,
    memory_budget_bytes: usize,
    strategy: ResidentIndexBuildStrategy,
}

#[must_use]
pub fn resident_index_engine_digest() -> String {
    let mut identity = blake3::Hasher::new();
    identity.update(b"agent-semantic-search-resident-index-engine-v1\0");
    identity.update(env!("CARGO_PKG_VERSION").as_bytes());
    identity.update(&[0]);
    identity.update(b"tantivy-0.26.1\0parallel-segments-no-merge\0");
    identity.update(crate::search_projection_analyzer_digest().as_bytes());
    identity.update(&[0]);
    identity
        .update(&(ResidentIndexBuildResources::TANTIVY_MAX_INDEXING_THREADS as u64).to_le_bytes());
    identity.update(
        &(ResidentIndexBuildResources::TANTIVY_MINIMUM_ARENA_BYTES_PER_THREAD as u64).to_le_bytes(),
    );
    format!("blake3-256:{}", identity.finalize().to_hex())
}

impl ResidentIndexBuildResources {
    // Tantivy 0.26 capability bounds, not ASP workload defaults. The ASP
    // Server supplies the actual envelope and this adapter derives a valid
    // worker topology from it.
    pub const TANTIVY_MAX_INDEXING_THREADS: usize = 8;
    pub const TANTIVY_MINIMUM_ARENA_BYTES_PER_THREAD: usize = 15_000_000;

    pub fn new(
        indexing_threads: usize,
        memory_budget_bytes: usize,
        strategy: ResidentIndexBuildStrategy,
    ) -> Result<Self, String> {
        if indexing_threads == 0 {
            return Err("resident index build requires at least one indexing thread".to_owned());
        }
        if indexing_threads > Self::TANTIVY_MAX_INDEXING_THREADS {
            return Err("resident index build exceeds Tantivy's indexing worker limit".to_owned());
        }
        if memory_budget_bytes == 0 {
            return Err("resident index build requires a nonzero memory budget".to_owned());
        }
        if memory_budget_bytes / indexing_threads < Self::TANTIVY_MINIMUM_ARENA_BYTES_PER_THREAD {
            return Err(
                "resident index build memory envelope cannot fund its indexing workers".to_owned(),
            );
        }
        Ok(Self {
            indexing_threads,
            memory_budget_bytes,
            strategy,
        })
    }

    #[must_use]
    pub fn indexing_threads(self) -> usize {
        self.indexing_threads
    }

    #[must_use]
    pub fn memory_budget_bytes(self) -> usize {
        self.memory_budget_bytes
    }

    #[must_use]
    pub fn strategy(self) -> ResidentIndexBuildStrategy {
        self.strategy
    }
}

/// Normalize lexical facts for a complete admitted owner batch with bounded
/// CPU parallelism while preserving input order.
///
/// This is the generation-construction API used by production and performance
/// qualification. Query execution never calls it.
#[must_use]
pub fn resident_lexical_coverage_batch(
    inputs: &[ResidentLexicalCoverageInput<'_>],
) -> Vec<Vec<String>> {
    if inputs.is_empty() {
        return Vec::new();
    }
    let worker_count = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .clamp(1, 8)
        .min(inputs.len());
    let chunk_size = inputs.len().div_ceil(worker_count);
    std::thread::scope(|scope| {
        inputs
            .chunks(chunk_size)
            .map(|chunk| {
                scope.spawn(move || {
                    chunk
                        .iter()
                        .map(|input| {
                            resident_lexical_coverage_keys(
                                input.owner_path,
                                input.source,
                                input.parser_query_keys.iter().cloned(),
                            )
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .flat_map(|worker| worker.join().expect("resident lexical coverage worker"))
            .collect()
    })
}

/// Derive lexical keys from already-admitted bytes for in-process fixtures and
/// provider adapters that explicitly own byte-token normalization.
///
/// Generation construction calls this once for added or changed owner bytes.
/// This function is not `rg` and its output must not be labeled as `rg`
/// evidence. Production generation admission joins real `fd` inventory and
/// caller-supplied `rg` facts through `plan_lexical_generation`. Warm queries
/// consume only the resulting resident keys and never read source.
#[must_use]
pub fn resident_lexical_coverage_keys(
    owner_path: &str,
    source: &[u8],
    parser_query_keys: impl IntoIterator<Item = String>,
) -> Vec<String> {
    let mut priority_keys = resident_navigation_keys(owner_path)
        .into_iter()
        .collect::<HashSet<_>>();
    for key in parser_query_keys {
        priority_keys.extend(source_index_lookup_terms(&key));
    }
    let mut source_keys = HashSet::new();
    let mut start = None;
    for (index, byte) in source
        .iter()
        .copied()
        .chain(std::iter::once(b' '))
        .enumerate()
    {
        let lexical = byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b':');
        match (start, lexical) {
            (None, true) => start = Some(index),
            (Some(begin), false) => {
                let token = &source[begin..index];
                if (2..=RESIDENT_LEXICAL_TOKEN_BYTES_LIMIT).contains(&token.len())
                    && token.iter().any(u8::is_ascii_alphabetic)
                {
                    insert_identifier_terms(&mut source_keys, &String::from_utf8_lossy(token));
                }
                start = None;
            }
            _ => {}
        }
    }
    source_keys.retain(|key| !priority_keys.contains(key));
    let mut keys = priority_keys.into_iter().collect::<Vec<_>>();
    keys.sort_unstable();
    if keys.len() < RESIDENT_LEXICAL_COVERAGE_KEY_LIMIT {
        let mut source_keys = source_keys.into_iter().collect::<Vec<_>>();
        source_keys.sort_unstable();
        keys.extend(
            source_keys
                .into_iter()
                .take(RESIDENT_LEXICAL_COVERAGE_KEY_LIMIT - keys.len()),
        );
    }
    keys.truncate(RESIDENT_LEXICAL_COVERAGE_KEY_LIMIT);
    keys
}

fn insert_identifier_terms(keys: &mut HashSet<String>, identifier: &str) {
    keys.insert(identifier.to_ascii_lowercase());
    for segment in identifier
        .split(['_', '-', ':'])
        .filter(|part| !part.is_empty())
    {
        let chars = segment.chars().collect::<Vec<_>>();
        let mut start = 0;
        for index in 1..chars.len() {
            let previous = chars[index - 1];
            let current = chars[index];
            let next = chars.get(index + 1).copied();
            let boundary = (previous.is_ascii_lowercase() || previous.is_ascii_digit())
                && current.is_ascii_uppercase()
                || previous.is_ascii_uppercase()
                    && current.is_ascii_uppercase()
                    && next.is_some_and(|next| next.is_ascii_lowercase());
            if boundary {
                insert_identifier_part(keys, &chars[start..index]);
                start = index;
            }
        }
        insert_identifier_part(keys, &chars[start..]);
    }
}

fn insert_identifier_part(keys: &mut HashSet<String>, chars: &[char]) {
    if chars.len() >= 2 {
        keys.insert(chars.iter().collect::<String>().to_ascii_lowercase());
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResidentSourceDocument {
    pub owner_path: String,
    pub owner_content_digest: String,
    pub line_count: u32,
    pub query_keys: Vec<String>,
    /// Admitted source text used only while building the immutable Tantivy
    /// attachment. Reopened attachments already contain their token positions.
    pub lexical_body: Option<String>,
    pub authority: Option<ResidentSearchAuthority>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct QueryCacheKey {
    query: String,
    authority: Option<ResidentSearchAuthority>,
    limit: u32,
}

#[derive(Debug)]
pub struct ResidentSourceIndex {
    lexical_index: crate::tantivy_lexical::TantivyLexicalIndex,
    owner_terms: Vec<Vec<String>>,
    source_documents: Vec<ResidentSourceDocument>,
    source_snapshot: SourceSnapshotEvidence,
    generation_digest: String,
    index_artifact_digest: String,
    query_cache: Vec<
        Mutex<
            Vec<(
                QueryCacheKey,
                Arc<agent_semantic_search_projection::ResidentSearchReadyResult>,
            )>,
        >,
    >,
}

impl ResidentSourceIndex {
    pub fn new(
        source_documents: BTreeMap<String, ResidentSourceDocument>,
        source_snapshot: SourceSnapshotEvidence,
        generation_digest: String,
        resources: ResidentIndexBuildResources,
    ) -> Result<Self, String> {
        let owner_terms = owner_terms(&source_documents);
        let lexical_documents = lexical_documents(&source_documents, &owner_terms);
        let lexical_index = crate::tantivy_lexical::TantivyLexicalIndex::build_with_resources(
            &lexical_documents,
            resources,
        )?;
        let index_artifact_digest =
            agent_semantic_search_projection::source_index_artifact_digest(&source_snapshot);
        Ok(Self::from_parts(
            lexical_index,
            owner_terms,
            source_documents,
            source_snapshot,
            generation_digest,
            index_artifact_digest,
        ))
    }

    /// Build and content-hash one immutable Tantivy generation artifact.
    ///
    /// Generation publication owns this method. Query/open paths must call
    /// `open_from_directory` and cannot silently rebuild the index.
    pub fn build_in_directory(
        directory: &Path,
        source_documents: BTreeMap<String, ResidentSourceDocument>,
        source_snapshot: SourceSnapshotEvidence,
        generation_digest: String,
        resources: ResidentIndexBuildResources,
    ) -> Result<(Self, String), String> {
        let owner_terms = owner_terms(&source_documents);
        let lexical_documents = lexical_documents(&source_documents, &owner_terms);
        let (lexical_index, artifact_digest) =
            crate::tantivy_lexical::TantivyLexicalIndex::build_in_directory(
                directory,
                &lexical_documents,
                resources,
            )?;
        let index = Self::from_parts(
            lexical_index,
            owner_terms,
            source_documents,
            source_snapshot,
            generation_digest,
            artifact_digest.clone(),
        );
        Ok((index, artifact_digest))
    }

    /// Open a content-proven Tantivy generation without tokenizing owners.
    pub fn open_from_directory(
        directory: &Path,
        expected_artifact_digest: &str,
        source_documents: BTreeMap<String, ResidentSourceDocument>,
        source_snapshot: SourceSnapshotEvidence,
        generation_digest: String,
    ) -> Result<Self, String> {
        let owner_terms = owner_terms(&source_documents);
        let lexical_index = crate::tantivy_lexical::TantivyLexicalIndex::open_from_directory(
            directory,
            owner_terms.len(),
            expected_artifact_digest,
        )?;
        Ok(Self::from_parts(
            lexical_index,
            owner_terms,
            source_documents,
            source_snapshot,
            generation_digest,
            expected_artifact_digest.to_owned(),
        ))
    }

    pub fn directory_artifact_digest(directory: &Path) -> Result<String, String> {
        crate::tantivy_lexical::TantivyLexicalIndex::artifact_digest(directory)
    }

    fn from_parts(
        lexical_index: crate::tantivy_lexical::TantivyLexicalIndex,
        owner_terms: Vec<Vec<String>>,
        source_documents: BTreeMap<String, ResidentSourceDocument>,
        source_snapshot: SourceSnapshotEvidence,
        generation_digest: String,
        index_artifact_digest: String,
    ) -> Self {
        Self {
            lexical_index,
            owner_terms,
            source_documents: source_documents.into_values().collect(),
            source_snapshot,
            generation_digest,
            index_artifact_digest,
            query_cache: (0..64).map(|_| Mutex::new(Vec::new())).collect(),
        }
    }

    pub fn query(
        &self,
        query: &str,
        authority: Option<&ResidentSearchAuthority>,
        limit: u32,
    ) -> Result<Arc<agent_semantic_search_projection::ResidentSearchReadyResult>, String> {
        if limit == 0 {
            return Err("resident source-index query limit must be non-zero".to_owned());
        }
        let cache_key = QueryCacheKey {
            query: query.trim().to_ascii_lowercase(),
            authority: authority.cloned(),
            limit,
        };
        let cache_slot = self.cache_slot(&cache_key);
        if let Some(result) = self.cached_result(cache_slot, &cache_key)? {
            return Ok(result);
        }
        let hits = self
            .owner_matches(query, authority, limit)?
            .into_iter()
            .map(|(owner_id, matched_terms)| self.candidate(owner_id, matched_terms))
            .collect::<Result<Vec<_>, String>>()?;
        let result = Arc::new(
            agent_semantic_search_projection::ResidentSearchReadyResult::new(
                self.generation_digest.clone(),
                &self.source_snapshot,
                self.index_artifact_digest.clone(),
                hits,
            )?,
        );
        let mut cache = self.query_cache[cache_slot]
            .lock()
            .map_err(|_| "resident source-index query cache is poisoned".to_owned())?;
        if cache.len() == 16 {
            cache.remove(0);
        }
        cache.push((cache_key, Arc::clone(&result)));
        Ok(result)
    }

    pub fn query_language(
        &self,
        query: &str,
        language_id: &agent_semantic_config::LanguageId,
        limit: u32,
    ) -> Result<Arc<agent_semantic_search_projection::ResidentSearchReadyResult>, String> {
        let mut authorities = self
            .source_documents
            .iter()
            .filter_map(|document| document.authority.as_ref())
            .filter(|authority| &authority.language_id == language_id);
        let authority = authorities.next().cloned().ok_or_else(|| {
            format!(
                "resident source-index language authority is missing: languageId={}",
                language_id.as_str()
            )
        })?;
        if authorities.any(|candidate| candidate.provider_id != authority.provider_id) {
            return Err(format!(
                "resident source-index language authority is ambiguous: languageId={}",
                language_id.as_str()
            ));
        }
        self.query(query, Some(&authority), limit)
    }

    /// Execute the public `--tantivy` grammar through Tantivy's native query
    /// parser. This is distinct from the compact term-intersection lookup used
    /// by internal navigation APIs.
    pub fn query_tantivy_language(
        &self,
        expression: &str,
        language_id: &agent_semantic_config::LanguageId,
        limit: u32,
    ) -> Result<Arc<agent_semantic_search_projection::ResidentSearchReadyResult>, String> {
        if expression.trim().is_empty() {
            return Err("native Tantivy expression must be non-empty".to_owned());
        }
        if limit == 0 {
            return Err("resident Tantivy query limit must be non-zero".to_owned());
        }
        let mut authorities = self
            .source_documents
            .iter()
            .filter_map(|document| document.authority.as_ref())
            .filter(|authority| &authority.language_id == language_id);
        let authority = authorities.next().cloned().ok_or_else(|| {
            format!(
                "resident source-index language authority is missing: languageId={}",
                language_id.as_str()
            )
        })?;
        if authorities.any(|candidate| candidate.provider_id != authority.provider_id) {
            return Err(format!(
                "resident source-index language authority is ambiguous: languageId={}",
                language_id.as_str()
            ));
        }
        let required_terms = [
            authority_language_term(&authority),
            authority_provider_term(&authority),
        ];
        let query_terms = source_index_lookup_terms(expression)
            .into_iter()
            .filter(|term| !term.chars().any(char::is_whitespace))
            .collect::<BTreeSet<_>>();
        let hits = self
            .lexical_index
            .search_expression(expression, &required_terms, limit as usize)?
            .into_iter()
            .filter(|owner_id| self.matches_authority(*owner_id, Some(&authority)))
            .map(|owner_id| {
                let matched_terms = query_terms
                    .iter()
                    .filter(|term| self.owner_terms[owner_id].binary_search(term).is_ok())
                    .cloned()
                    .collect();
                self.candidate(owner_id, matched_terms)
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Arc::new(
            agent_semantic_search_projection::ResidentSearchReadyResult::new(
                self.generation_digest.clone(),
                &self.source_snapshot,
                self.index_artifact_digest.clone(),
                hits,
            )?,
        ))
    }

    pub fn result_for_owner_paths(
        &self,
        query: &str,
        owner_paths: &[String],
        authority: Option<&ResidentSearchAuthority>,
        limit: u32,
    ) -> Result<Arc<agent_semantic_search_projection::ResidentSearchReadyResult>, String> {
        if !(1..=100).contains(&limit) {
            return Err("resident exact-byte result limit must be in 1..=100".to_owned());
        }
        let admitted = owner_paths.iter().collect::<BTreeSet<_>>();
        let matched_term = query.to_owned();
        let hits = self
            .source_documents
            .iter()
            .enumerate()
            .filter(|(owner_id, document)| {
                admitted.contains(&document.owner_path)
                    && self.matches_authority(*owner_id, authority)
            })
            .take(limit as usize)
            .map(|(owner_id, _)| self.candidate(owner_id, vec![matched_term.clone()]))
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Arc::new(
            agent_semantic_search_projection::ResidentSearchReadyResult::new(
                self.generation_digest.clone(),
                &self.source_snapshot,
                self.index_artifact_digest.clone(),
                hits,
            )?,
        ))
    }

    fn cache_slot(&self, cache_key: &QueryCacheKey) -> usize {
        let language_id = cache_key
            .authority
            .as_ref()
            .map_or("", |authority| authority.language_id.as_str());
        let provider_id = cache_key
            .authority
            .as_ref()
            .map_or("", |authority| authority.provider_id.as_str());
        let mut hasher = blake3::Hasher::new();
        hasher.update(cache_key.query.as_bytes());
        hasher.update(&[0]);
        hasher.update(language_id.as_bytes());
        hasher.update(&[0]);
        hasher.update(provider_id.as_bytes());
        hasher.update(&[0]);
        hasher.update(&cache_key.limit.to_le_bytes());
        let digest = hasher.finalize();
        usize::from(u16::from_le_bytes([
            digest.as_bytes()[0],
            digest.as_bytes()[1],
        ])) % self.query_cache.len()
    }

    fn cached_result(
        &self,
        cache_slot: usize,
        cache_key: &QueryCacheKey,
    ) -> Result<Option<Arc<agent_semantic_search_projection::ResidentSearchReadyResult>>, String>
    {
        Ok(self.query_cache[cache_slot]
            .lock()
            .map_err(|_| "resident source-index query cache is poisoned".to_owned())?
            .iter()
            .find(|(stored_key, _)| stored_key == cache_key)
            .map(|(_, result)| Arc::clone(result)))
    }

    fn owner_matches(
        &self,
        query: &str,
        authority: Option<&ResidentSearchAuthority>,
        limit: u32,
    ) -> Result<Vec<(usize, Vec<String>)>, String> {
        let normalized = query.trim().to_ascii_lowercase();
        let query_terms = if is_exact_lexical_query(&normalized) {
            vec![normalized]
        } else {
            source_index_lookup_terms(query)
                .into_iter()
                // `source_index_lookup_terms` also preserves the whole query for
                // exact/path ranking.  A multi-token phrase is not a Tantivy
                // posting and must not become an extra mandatory intersection
                // term, otherwise every ordinary playbook query is empty.
                .filter(|term| !term.chars().any(char::is_whitespace))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .take(64)
                .collect::<Vec<_>>()
        };
        let mut search_terms = query_terms.clone();
        if let Some(authority) = authority {
            search_terms.push(authority_language_term(authority));
            search_terms.push(authority_provider_term(authority));
        }
        Ok(self
            .lexical_index
            .search(&search_terms, limit as usize)?
            .into_iter()
            .filter(|owner_id| self.matches_authority(*owner_id, authority))
            .take(limit as usize)
            .map(|owner_id| {
                let matched_terms = query_terms
                    .iter()
                    .filter(|term| self.owner_terms[owner_id].binary_search(term).is_ok())
                    .cloned()
                    .collect();
                (owner_id, matched_terms)
            })
            .collect::<Vec<_>>())
    }

    fn matches_authority(
        &self,
        owner_id: usize,
        requested: Option<&ResidentSearchAuthority>,
    ) -> bool {
        requested.is_none_or(|requested| {
            self.source_documents
                .get(owner_id)
                .and_then(|document| document.authority.as_ref())
                == Some(requested)
        })
    }

    fn candidate(
        &self,
        owner_id: usize,
        matched_terms: Vec<String>,
    ) -> Result<agent_semantic_search_projection::ResidentSearchHit, String> {
        let document = self.source_documents.get(owner_id).ok_or_else(|| {
            "resident source-index lexical index references a missing owner".to_owned()
        })?;
        Ok(agent_semantic_search_projection::ResidentSearchHit {
            owner_path: document.owner_path.clone(),
            owner_content_digest: document.owner_content_digest.clone(),
            language_id: document
                .authority
                .as_ref()
                .map(|authority| authority.language_id.as_str().to_owned()),
            projection_tier:
                agent_semantic_search_projection::ResidentSearchProjectionTier::ShallowNavigation,
            line_count: document.line_count,
            query_keys: matched_terms,
            selector: None,
            score: None,
        })
    }
}

fn owner_terms(source_documents: &BTreeMap<String, ResidentSourceDocument>) -> Vec<Vec<String>> {
    source_documents
        .values()
        .map(|document| {
            let mut terms = document.query_keys.clone();
            if let Some(authority) = &document.authority {
                terms.push(authority_language_term(authority));
                terms.push(authority_provider_term(authority));
            }
            terms.sort_unstable();
            terms.dedup();
            terms
        })
        .collect()
}

fn lexical_documents(
    source_documents: &BTreeMap<String, ResidentSourceDocument>,
    owner_terms: &[Vec<String>],
) -> Vec<crate::tantivy_lexical::TantivyLexicalDocument> {
    source_documents
        .values()
        .zip(owner_terms)
        .map(
            |(document, exact_terms)| crate::tantivy_lexical::TantivyLexicalDocument {
                exact_terms: exact_terms.clone(),
                title: document.owner_path.clone(),
                body: document
                    .lexical_body
                    .clone()
                    .unwrap_or_else(|| document.query_keys.join(" ")),
            },
        )
        .collect()
}

fn authority_language_term(authority: &ResidentSearchAuthority) -> String {
    format!("__asp_language__{}", authority.language_id.as_str())
}

fn authority_provider_term(authority: &ResidentSearchAuthority) -> String {
    format!("__asp_provider__{}", authority.provider_id.as_str())
}

fn is_exact_lexical_query(query: &str) -> bool {
    !query.is_empty()
        && query
            .chars()
            .all(|character| character.is_alphanumeric() || character == '-' || character == '_')
        && query.contains(['-', '_'])
}

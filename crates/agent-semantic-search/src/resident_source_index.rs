//! Immutable, process-local source-index query data plane.
//!
//! Construction belongs to generation admission. Query methods perform no
//! filesystem, database, provider, socket, or scheduler work.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap};
use std::sync::{Arc, Mutex};

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

/// Build the immutable lexical coverage for one admitted owner.
///
/// Generation construction calls this once for added or changed owner bytes.
/// It is the in-process equivalent of ripgrep's fast lexical coverage stage:
/// warm queries consume only the resulting mmap keys and never spawn `rg` or
/// read source. Parser-owned selector keys are folded into the same set so the
/// resident index has one lexical authority.
#[must_use]
pub fn resident_lexical_coverage_keys(
    owner_path: &str,
    source: &[u8],
    parser_query_keys: impl IntoIterator<Item = String>,
) -> Vec<String> {
    let mut priority_keys = resident_navigation_keys(owner_path)
        .into_iter()
        .collect::<BTreeSet<_>>();
    for key in parser_query_keys {
        priority_keys.extend(source_index_lookup_terms(&key));
    }
    let mut source_keys = BTreeSet::new();
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
    if keys.len() < RESIDENT_LEXICAL_COVERAGE_KEY_LIMIT {
        keys.extend(
            source_keys
                .into_iter()
                .take(RESIDENT_LEXICAL_COVERAGE_KEY_LIMIT - keys.len()),
        );
    }
    keys.truncate(RESIDENT_LEXICAL_COVERAGE_KEY_LIMIT);
    keys
}

fn insert_identifier_terms(keys: &mut BTreeSet<String>, identifier: &str) {
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

fn insert_identifier_part(keys: &mut BTreeSet<String>, chars: &[char]) {
    if chars.len() >= 2 {
        keys.insert(chars.iter().collect::<String>().to_ascii_lowercase());
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResidentSourceIndexSeed {
    pub owner_path: String,
    pub owner_content_digest: String,
    pub line_count: u32,
    pub query_keys: Vec<String>,
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
    lexical_index: BTreeMap<String, Vec<usize>>,
    candidate_seeds: Vec<ResidentSourceIndexSeed>,
    source_snapshot: SourceSnapshotEvidence,
    generation_digest: String,
    index_artifact_digest: String,
    query_cache: Vec<
        Mutex<
            Option<(
                QueryCacheKey,
                Arc<agent_semantic_search_projection::ResidentSearchReadyResult>,
            )>,
        >,
    >,
}

impl ResidentSourceIndex {
    #[must_use]
    pub fn new(
        lexical_index: BTreeMap<String, Vec<String>>,
        candidate_seeds: BTreeMap<String, ResidentSourceIndexSeed>,
        source_snapshot: SourceSnapshotEvidence,
        generation_digest: String,
    ) -> Self {
        let index_artifact_digest =
            agent_semantic_search_projection::source_index_artifact_digest(&source_snapshot);
        let owner_ids = candidate_seeds
            .keys()
            .enumerate()
            .map(|(owner_id, owner_path)| (owner_path.clone(), owner_id))
            .collect::<BTreeMap<_, _>>();
        let lexical_index = lexical_index
            .into_iter()
            .map(|(term, owner_paths)| {
                let mut postings = owner_paths
                    .into_iter()
                    .map(|owner_path| owner_ids.get(&owner_path).copied().unwrap_or(usize::MAX))
                    .collect::<Vec<_>>();
                postings.sort_unstable();
                postings.dedup();
                (term, postings)
            })
            .collect();
        Self {
            lexical_index,
            candidate_seeds: candidate_seeds.into_values().collect(),
            source_snapshot,
            generation_digest,
            index_artifact_digest,
            query_cache: (0..64).map(|_| Mutex::new(None)).collect(),
        }
    }

    pub fn query(
        &self,
        query: &str,
        authority: Option<&ResidentSearchAuthority>,
        limit: u32,
    ) -> Result<Arc<agent_semantic_search_projection::ResidentSearchReadyResult>, String> {
        if !(1..=100).contains(&limit) {
            return Err(format!(
                "resident source-index query limit must be in 1..=100: limit={limit}"
            ));
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
            .owner_matches(query, authority, limit)
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
        self.query_cache[cache_slot]
            .lock()
            .map_err(|_| "resident source-index query cache is poisoned".to_owned())?
            .replace((cache_key, Arc::clone(&result)));
        Ok(result)
    }

    pub fn query_language(
        &self,
        query: &str,
        language_id: &agent_semantic_config::LanguageId,
        limit: u32,
    ) -> Result<Arc<agent_semantic_search_projection::ResidentSearchReadyResult>, String> {
        let mut authorities = self
            .candidate_seeds
            .iter()
            .filter_map(|seed| seed.authority.as_ref())
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
            .as_ref()
            .filter(|(stored_key, _)| stored_key == cache_key)
            .map(|(_, result)| Arc::clone(result)))
    }

    fn owner_matches(
        &self,
        query: &str,
        authority: Option<&ResidentSearchAuthority>,
        limit: u32,
    ) -> Vec<(usize, Vec<String>)> {
        let normalized = query.trim().to_ascii_lowercase();
        if is_exact_lexical_query(&normalized) {
            return self
                .lexical_index
                .get(&normalized)
                .into_iter()
                .flatten()
                .copied()
                .filter(|owner_id| self.matches_authority(*owner_id, authority))
                .take(limit as usize)
                .map(|owner_id| (owner_id, vec![normalized.clone()]))
                .collect();
        }
        let query_terms = source_index_lookup_terms(query)
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .take(64)
            .collect::<Vec<_>>();
        let mut scores = HashMap::<usize, u16>::new();
        for owner_id in query_terms
            .iter()
            .filter_map(|term| self.lexical_index.get(term))
            .flatten()
            .copied()
            .filter(|owner_id| self.matches_authority(*owner_id, authority))
        {
            let score = scores.entry(owner_id).or_default();
            *score = score.saturating_add(1);
        }
        let frontier_limit = limit as usize;
        if frontier_limit == 0 {
            return Vec::new();
        }
        let mut frontier = BinaryHeap::<(Reverse<u16>, usize)>::with_capacity(frontier_limit);
        for (owner_id, score) in scores {
            let ranked_owner = (Reverse(score), owner_id);
            if frontier.len() < frontier_limit {
                frontier.push(ranked_owner);
                continue;
            }
            if frontier.peek().is_some_and(|worst| ranked_owner < *worst) {
                frontier.pop();
                frontier.push(ranked_owner);
            }
        }
        let mut ranked = frontier.into_vec();
        ranked.sort_unstable_by(|left, right| {
            right.0.0.cmp(&left.0.0).then_with(|| left.1.cmp(&right.1))
        });
        ranked
            .into_iter()
            .map(|(_, owner_id)| {
                let matched_terms = query_terms
                    .iter()
                    .filter(|term| {
                        self.lexical_index
                            .get(*term)
                            .is_some_and(|postings| postings.binary_search(&owner_id).is_ok())
                    })
                    .cloned()
                    .collect();
                (owner_id, matched_terms)
            })
            .collect()
    }

    fn matches_authority(
        &self,
        owner_id: usize,
        requested: Option<&ResidentSearchAuthority>,
    ) -> bool {
        requested.is_none_or(|requested| {
            self.candidate_seeds
                .get(owner_id)
                .and_then(|seed| seed.authority.as_ref())
                == Some(requested)
        })
    }

    fn candidate(
        &self,
        owner_id: usize,
        matched_terms: Vec<String>,
    ) -> Result<agent_semantic_search_projection::ResidentSearchHit, String> {
        let seed = self.candidate_seeds.get(owner_id).ok_or_else(|| {
            "resident source-index lexical index references a missing owner".to_owned()
        })?;
        Ok(agent_semantic_search_projection::ResidentSearchHit {
            owner_path: seed.owner_path.clone(),
            owner_content_digest: seed.owner_content_digest.clone(),
            language_id: seed
                .authority
                .as_ref()
                .map(|authority| authority.language_id.as_str().to_owned()),
            projection_tier:
                agent_semantic_search_projection::ResidentSearchProjectionTier::ShallowNavigation,
            line_count: seed.line_count,
            query_keys: matched_terms,
            selector: None,
            score: None,
        })
    }
}

fn is_exact_lexical_query(query: &str) -> bool {
    !query.is_empty()
        && query
            .chars()
            .all(|character| character.is_alphanumeric() || character == '-' || character == '_')
        && query.contains(['-', '_'])
}

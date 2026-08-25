//! Immutable, process-local source-index query data plane.
//!
//! Construction belongs to generation admission. Query methods perform no
//! filesystem, database, provider, socket, or scheduler work.

use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

use agent_semantic_content_identity::SourceSnapshotEvidence;

use crate::source_index_lookup_terms;

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResidentSearchAuthority {
    pub language_id: agent_semantic_client_core::LanguageId,
    pub provider_id: agent_semantic_client_core::ProviderId,
}

/// Stable navigation keys admitted to the durable shallow search projection.
///
/// Source text and nested syntax items are intentionally excluded. Those facts
/// belong to owner-local dynamic projections keyed by the owner content digest.
#[must_use]
pub fn resident_navigation_keys(owner_path: &str) -> Vec<String> {
    source_index_lookup_terms(owner_path)
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
    lexical_index: BTreeMap<String, Vec<String>>,
    candidate_seeds: BTreeMap<String, ResidentSourceIndexSeed>,
    source_snapshot: SourceSnapshotEvidence,
    generation_digest: String,
    index_artifact_digest: String,
    query_cache: Vec<
        Mutex<
            Option<(
                QueryCacheKey,
                agent_semantic_search_projection::ResidentSearchReadyResult,
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
        Self {
            lexical_index,
            candidate_seeds,
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
    ) -> Result<agent_semantic_search_projection::ResidentSearchReadyResult, String> {
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
            .owner_paths(query, authority, limit)
            .into_iter()
            .map(|owner_path| self.candidate(&owner_path))
            .collect::<Result<Vec<_>, String>>()?;
        let result = agent_semantic_search_projection::ResidentSearchReadyResult::new(
            self.generation_digest.clone(),
            &self.source_snapshot,
            self.index_artifact_digest.clone(),
            hits,
        )?;
        self.query_cache[cache_slot]
            .lock()
            .map_err(|_| "resident source-index query cache is poisoned".to_owned())?
            .replace((cache_key, result.clone()));
        Ok(result)
    }

    pub fn query_language(
        &self,
        query: &str,
        language_id: &agent_semantic_client_core::LanguageId,
        limit: u32,
    ) -> Result<agent_semantic_search_projection::ResidentSearchReadyResult, String> {
        let mut authorities = self
            .candidate_seeds
            .values()
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
    ) -> Result<Option<agent_semantic_search_projection::ResidentSearchReadyResult>, String> {
        Ok(self.query_cache[cache_slot]
            .lock()
            .map_err(|_| "resident source-index query cache is poisoned".to_owned())?
            .as_ref()
            .filter(|(stored_key, _)| stored_key == cache_key)
            .map(|(_, result)| result.clone()))
    }

    fn owner_paths(
        &self,
        query: &str,
        authority: Option<&ResidentSearchAuthority>,
        limit: u32,
    ) -> Vec<String> {
        let normalized = query.trim().to_ascii_lowercase();
        if is_exact_lexical_query(&normalized) {
            return self
                .lexical_index
                .get(&normalized)
                .into_iter()
                .flatten()
                .filter(|path| self.matches_authority(path, authority))
                .take(limit as usize)
                .cloned()
                .collect();
        }
        let scores = source_index_lookup_terms(query)
            .into_iter()
            .filter_map(|term| self.lexical_index.get(&term))
            .flatten()
            .filter(|path| self.matches_authority(path, authority))
            .fold(HashMap::<&str, usize>::new(), |mut scores, path| {
                *scores.entry(path.as_str()).or_default() += 1;
                scores
            });
        let mut ranked = scores.into_iter().collect::<Vec<_>>();
        ranked
            .sort_unstable_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(right.0)));
        ranked.truncate(limit as usize);
        ranked
            .into_iter()
            .map(|(path, _)| path.to_owned())
            .collect()
    }

    fn matches_authority(
        &self,
        owner_path: &str,
        requested: Option<&ResidentSearchAuthority>,
    ) -> bool {
        requested.is_none_or(|requested| {
            self.candidate_seeds
                .get(owner_path)
                .and_then(|seed| seed.authority.as_ref())
                == Some(requested)
        })
    }

    fn candidate(
        &self,
        owner_path: &str,
    ) -> Result<agent_semantic_search_projection::ResidentSearchHit, String> {
        let seed = self.candidate_seeds.get(owner_path).ok_or_else(|| {
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
            query_keys: seed.query_keys.clone(),
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

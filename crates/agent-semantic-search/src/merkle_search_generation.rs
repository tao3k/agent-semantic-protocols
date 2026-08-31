//! Immutable Merkle generation shared by lexical and graph projections.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::{ResidentSearchAuthority, ResidentSourceIndex, ResidentSourceIndexSeed};

const SEARCH_PROJECTION_ANALYZER_ID: &str =
    "agent.semantic-protocols.search-projection-analyzer.lexical-graph.v1";

/// Digest of the parser-owned derivation algorithm shared by lexical and graph projections.
///
/// This is deliberately independent from a Runtime activation epoch. Changing lexical token
/// semantics or graph ownership rules requires a new analyzer identity and therefore cannot reuse
/// fragments produced by the previous algorithm.
#[must_use]
pub fn search_projection_analyzer_digest() -> String {
    format!(
        "blake3-256:{}",
        blake3::hash(SEARCH_PROJECTION_ANALYZER_ID.as_bytes()).to_hex()
    )
}

/// Canonical digest for every graph edge attributed to one parser-owned source owner.
pub fn search_owner_graph_fragment_digest(
    relations: &[agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelation],
) -> Result<String, String> {
    let mut canonical = relations.to_vec();
    for relation in &canonical {
        relation.validate()?;
    }
    canonical.sort();
    canonical.dedup();
    let bytes = serde_json::to_vec(&canonical)
        .map_err(|error| format!("encode search owner graph fragment: {error}"))?;
    let mut hasher = blake3::Hasher::new();
    hash_field(
        &mut hasher,
        "agent.semantic-protocols.search-owner-graph-fragment.v1",
    );
    hasher.update(&(bytes.len() as u64).to_le_bytes());
    hasher.update(&bytes);
    Ok(format!("blake3-256:{}", hasher.finalize().to_hex()))
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchProjectionIdentity {
    pub workspace_identity: String,
    pub source_root_digest: String,
    pub provider_digest: String,
    pub schema_digest: String,
    pub analyzer_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchOwnerFragment {
    pub owner_path: String,
    pub content_digest: String,
    pub line_count: u32,
    pub lexical_query_keys: Vec<String>,
    pub graph_fragment_digest: Option<String>,
}

impl SearchOwnerFragment {
    pub fn new(
        owner_path: impl Into<String>,
        content_digest: impl Into<String>,
        line_count: u32,
        lexical_query_keys: Vec<String>,
        graph_fragment_digest: Option<String>,
    ) -> Result<Self, String> {
        let owner_path = owner_path.into();
        let content_digest = content_digest.into();
        if owner_path.is_empty() || content_digest.is_empty() {
            return Err("search owner fragment requires path and content digest".to_owned());
        }
        let mut canonical_keys = lexical_query_keys;
        canonical_keys.sort_unstable();
        canonical_keys.dedup();
        if canonical_keys.iter().any(String::is_empty) {
            return Err("search owner fragment query keys must be non-empty".to_owned());
        }
        if graph_fragment_digest.as_deref() == Some("") {
            return Err("search owner graph fragment digest must be non-empty".to_owned());
        }
        Ok(Self {
            owner_path,
            content_digest,
            line_count,
            lexical_query_keys: canonical_keys,
            graph_fragment_digest,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchOwnerChange {
    Added {
        fragment: SearchOwnerFragment,
    },
    Changed {
        previous_content_digest: String,
        fragment: SearchOwnerFragment,
    },
    Removed {
        owner_path: String,
        previous_content_digest: String,
    },
}

impl SearchOwnerChange {
    fn owner_path(&self) -> &str {
        match self {
            Self::Added { fragment } | Self::Changed { fragment, .. } => &fragment.owner_path,
            Self::Removed { owner_path, .. } => owner_path,
        }
    }
}

#[derive(Clone, Debug)]
pub struct MerkleSearchGeneration {
    identity: SearchProjectionIdentity,
    owners: BTreeMap<String, Arc<SearchOwnerFragment>>,
    tombstones: BTreeSet<String>,
    manifest_digest: String,
}

impl MerkleSearchGeneration {
    pub fn apply_change_set(
        base: Option<&Self>,
        identity: SearchProjectionIdentity,
        changes: Vec<SearchOwnerChange>,
    ) -> Result<Self, String> {
        validate_identity(base, &identity)?;
        validate_change_order(&changes)?;
        let mut owners = base.map_or_else(BTreeMap::new, |base| base.owners.clone());
        let mut tombstones = base.map_or_else(BTreeSet::new, |base| base.tombstones.clone());
        for change in changes {
            match change {
                SearchOwnerChange::Added { fragment } => {
                    if owners.contains_key(&fragment.owner_path) {
                        return Err(format!(
                            "added search owner already exists: {}",
                            fragment.owner_path
                        ));
                    }
                    tombstones.remove(&fragment.owner_path);
                    owners.insert(fragment.owner_path.clone(), Arc::new(fragment));
                }
                SearchOwnerChange::Changed {
                    previous_content_digest,
                    fragment,
                } => {
                    require_previous_digest(
                        &owners,
                        &fragment.owner_path,
                        &previous_content_digest,
                    )?;
                    tombstones.remove(&fragment.owner_path);
                    owners.insert(fragment.owner_path.clone(), Arc::new(fragment));
                }
                SearchOwnerChange::Removed {
                    owner_path,
                    previous_content_digest,
                } => {
                    require_previous_digest(&owners, &owner_path, &previous_content_digest)?;
                    owners.remove(&owner_path);
                    tombstones.insert(owner_path);
                }
            }
        }
        let manifest_digest = generation_manifest_digest(&identity, &owners, &tombstones);
        Ok(Self {
            identity,
            owners,
            tombstones,
            manifest_digest,
        })
    }

    #[must_use]
    pub fn identity(&self) -> &SearchProjectionIdentity {
        &self.identity
    }

    #[must_use]
    pub fn manifest_digest(&self) -> &str {
        &self.manifest_digest
    }

    #[must_use]
    pub fn owner(&self, owner_path: &str) -> Option<&Arc<SearchOwnerFragment>> {
        self.owners.get(owner_path)
    }

    #[must_use]
    pub fn tombstones(&self) -> &BTreeSet<String> {
        &self.tombstones
    }

    #[must_use]
    pub fn owner_count(&self) -> usize {
        self.owners.len()
    }

    pub fn resident_source_index_from_seeds(
        &self,
        source_snapshot: agent_semantic_artifacts::SourceSnapshotEvidence,
        generation_digest: String,
        candidate_seeds: BTreeMap<String, ResidentSourceIndexSeed>,
    ) -> Result<ResidentSourceIndex, String> {
        if source_snapshot.root_digest != self.identity.source_root_digest
            || source_snapshot.provider_digest != self.identity.provider_digest
        {
            return Err(
                "Merkle search generation and source snapshot identity mismatch".to_owned(),
            );
        }
        if candidate_seeds.len() != self.owners.len() {
            return Err("Merkle search generation owner seed coverage mismatch".to_owned());
        }
        let mut lexical_index = BTreeMap::<String, Vec<String>>::new();
        for (owner_path, fragment) in &self.owners {
            let seed = candidate_seeds.get(owner_path).ok_or_else(|| {
                format!("Merkle search generation omitted owner seed: {owner_path}")
            })?;
            let mut seed_keys = seed.query_keys.clone();
            seed_keys.sort_unstable();
            seed_keys.dedup();
            if seed.owner_path != *owner_path
                || seed.owner_content_digest != fragment.content_digest
                || seed.line_count != fragment.line_count
                || seed_keys != fragment.lexical_query_keys
            {
                return Err(format!(
                    "Merkle search generation owner projection drift: {owner_path}"
                ));
            }
            for key in &fragment.lexical_query_keys {
                lexical_index
                    .entry(key.clone())
                    .or_default()
                    .push(owner_path.clone());
            }
        }
        if candidate_seeds
            .keys()
            .any(|owner_path| !self.owners.contains_key(owner_path))
        {
            return Err("Merkle search generation contains an uncommitted owner seed".to_owned());
        }
        Ok(ResidentSourceIndex::new(
            lexical_index,
            candidate_seeds,
            source_snapshot,
            generation_digest,
        ))
    }

    pub fn resident_source_index(
        &self,
        source_snapshot: agent_semantic_artifacts::SourceSnapshotEvidence,
        language_id: agent_semantic_client_core::LanguageId,
        provider_id: agent_semantic_client_core::ProviderId,
    ) -> Result<ResidentSourceIndex, String> {
        if source_snapshot.root_digest != self.identity.source_root_digest
            || source_snapshot.provider_digest != self.identity.provider_digest
        {
            return Err(
                "Merkle search generation and source snapshot identity mismatch".to_owned(),
            );
        }
        let authority = ResidentSearchAuthority {
            language_id,
            provider_id,
        };
        let mut lexical_index = BTreeMap::<String, Vec<String>>::new();
        let candidate_seeds = self
            .owners
            .values()
            .map(|fragment| {
                for key in &fragment.lexical_query_keys {
                    lexical_index
                        .entry(key.clone())
                        .or_default()
                        .push(fragment.owner_path.clone());
                }
                (
                    fragment.owner_path.clone(),
                    ResidentSourceIndexSeed {
                        owner_path: fragment.owner_path.clone(),
                        owner_content_digest: fragment.content_digest.clone(),
                        line_count: fragment.line_count,
                        query_keys: fragment.lexical_query_keys.clone(),
                        authority: Some(authority.clone()),
                    },
                )
            })
            .collect();
        self.resident_source_index_from_seeds(
            source_snapshot,
            self.manifest_digest.clone(),
            candidate_seeds,
        )
    }
}

fn validate_identity(
    base: Option<&MerkleSearchGeneration>,
    candidate: &SearchProjectionIdentity,
) -> Result<(), String> {
    if candidate.workspace_identity.is_empty()
        || candidate.source_root_digest.is_empty()
        || candidate.provider_digest.is_empty()
        || candidate.schema_digest.is_empty()
        || candidate.analyzer_digest.is_empty()
    {
        return Err("Merkle search generation identity fields must be non-empty".to_owned());
    }
    if let Some(base) = base {
        let previous = &base.identity;
        if previous.workspace_identity != candidate.workspace_identity
            || previous.provider_digest != candidate.provider_digest
            || previous.schema_digest != candidate.schema_digest
            || previous.analyzer_digest != candidate.analyzer_digest
        {
            return Err(
                "Merkle search generation authority changed across a change set".to_owned(),
            );
        }
        if previous.source_root_digest == candidate.source_root_digest {
            return Err("Merkle search change set must advance the source root".to_owned());
        }
    }
    Ok(())
}

fn validate_change_order(changes: &[SearchOwnerChange]) -> Result<(), String> {
    if changes
        .windows(2)
        .any(|pair| pair[0].owner_path() >= pair[1].owner_path())
    {
        return Err("Merkle search owner changes must be unique and path-sorted".to_owned());
    }
    Ok(())
}

fn require_previous_digest(
    owners: &BTreeMap<String, Arc<SearchOwnerFragment>>,
    owner_path: &str,
    previous_content_digest: &str,
) -> Result<(), String> {
    let current = owners
        .get(owner_path)
        .ok_or_else(|| format!("search owner is absent from base generation: {owner_path}"))?;
    if current.content_digest != previous_content_digest {
        return Err(format!(
            "search owner previous digest mismatch: {owner_path}"
        ));
    }
    Ok(())
}

fn generation_manifest_digest(
    identity: &SearchProjectionIdentity,
    owners: &BTreeMap<String, Arc<SearchOwnerFragment>>,
    tombstones: &BTreeSet<String>,
) -> String {
    let mut hasher = blake3::Hasher::new();
    hash_field(
        &mut hasher,
        "agent.semantic-protocols.merkle-search-generation.v1",
    );
    for value in [
        &identity.workspace_identity,
        &identity.source_root_digest,
        &identity.provider_digest,
        &identity.schema_digest,
        &identity.analyzer_digest,
    ] {
        hash_field(&mut hasher, "identity");
        hash_field(&mut hasher, value);
    }
    for (owner_path, fragment) in owners {
        hash_field(&mut hasher, "owner");
        hash_field(&mut hasher, owner_path);
        hash_field(&mut hasher, &fragment.content_digest);
        hasher.update(&fragment.line_count.to_le_bytes());
        for key in &fragment.lexical_query_keys {
            hash_field(&mut hasher, "query-key");
            hash_field(&mut hasher, key);
        }
        hash_field(&mut hasher, "graph-fragment");
        hash_field(
            &mut hasher,
            fragment.graph_fragment_digest.as_deref().unwrap_or(""),
        );
    }
    for owner_path in tombstones {
        hash_field(&mut hasher, "tombstone");
        hash_field(&mut hasher, owner_path);
    }
    format!("blake3-256:{}", hasher.finalize().to_hex())
}

fn hash_field(hasher: &mut blake3::Hasher, value: &str) {
    hasher.update(&(value.len() as u64).to_le_bytes());
    hasher.update(value.as_bytes());
}

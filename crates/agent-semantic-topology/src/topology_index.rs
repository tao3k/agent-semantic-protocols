// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Content-bound resident index over the repository topology skeleton.
//!
//! The input surface can represent owners and parser-native nodes, but not
//! source or prose bodies. Symbols and headings are peer node features.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use agent_semantic_content_identity::CanonicalItemSelector;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
/// Language-neutral kind of one indexed topology node.
pub enum TopologyNodeKindV1 {
    /// One directory derived from an admitted owner path.
    Directory,
    /// One admitted source/document owner.
    Owner,
    /// One canonical provider-native item such as Function, Module, or Heading.
    ParserNative {
        language_id: String,
        native_kind: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// One parser-native topology node and its searchable features.
pub struct TopologyNodeV1 {
    pub structural_selector: String,
    pub kind: TopologyNodeKindV1,
    pub features: Vec<String>,
}

impl TopologyNodeV1 {
    /// Decode canonical language/kind identity from an exact selector.
    pub fn from_selector(
        structural_selector: impl Into<String>,
        features: Vec<String>,
    ) -> Result<Self, String> {
        let structural_selector = structural_selector.into();
        let selector =
            CanonicalItemSelector::parse_root_or_exact_descendant(structural_selector.clone())?;
        Ok(Self {
            structural_selector,
            kind: TopologyNodeKindV1::ParserNative {
                language_id: selector.language_id.as_str().to_owned(),
                native_kind: selector.kind.as_str().to_owned(),
            },
            features,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// One content-bound owner shard supplied by any language/document provider.
pub struct TopologyOwnerV1 {
    pub owner_path: String,
    pub owner_content_digest: String,
    pub nodes: Vec<TopologyNodeV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Compact resident lookup result for navigation or exact Query handoff.
pub struct TopologyHitV1 {
    pub owner_path: String,
    pub owner_content_digest: String,
    /// Directory path, owner path, or canonical selector.
    pub topology_locator: String,
    pub kind: TopologyNodeKindV1,
    pub structural_selector: Option<String>,
    pub matched_features: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Exact parser-native selector proven by ranked text and topology postings.
pub struct RankedTextTopologySelectorHitV1 {
    pub owner_path: String,
    pub structural_selector: String,
}

#[derive(Debug, Default)]
/// Immutable postings over directories, owners, and parser-native nodes.
pub struct TopologyIndexV1 {
    postings: BTreeMap<String, Arc<[TopologyHitV1]>>,
    owner_shard_digests: BTreeMap<String, String>,
    node_count: usize,
    parser_native_node_count: usize,
}

impl TopologyIndexV1 {
    /// Build deterministic owner shards and rarest-first feature postings.
    pub fn build(owners: impl IntoIterator<Item = TopologyOwnerV1>) -> Result<Self, String> {
        let mut postings = BTreeMap::<String, Vec<TopologyHitV1>>::new();
        let mut owner_shard_digests = BTreeMap::new();
        let mut node_count = 0usize;
        let mut parser_native_node_count = 0usize;
        for mut owner in owners {
            validate_owner(&owner)?;
            for node in &mut owner.nodes {
                node.features.sort_unstable();
                node.features.dedup();
            }
            owner
                .nodes
                .sort_by(|left, right| left.structural_selector.cmp(&right.structural_selector));
            let shard_digest = topology_shard_digest(&owner);
            if owner_shard_digests
                .insert(owner.owner_path.clone(), shard_digest)
                .is_some()
            {
                return Err("topology index contains a duplicate owner path".to_owned());
            }

            for directory in owner_directories(&owner.owner_path) {
                node_count = node_count.saturating_add(1);
                add_postings(
                    &mut postings,
                    &owner,
                    directory.clone(),
                    TopologyNodeKindV1::Directory,
                    None,
                    topology_feature_terms(&directory),
                );
            }
            node_count = node_count.saturating_add(1);
            add_postings(
                &mut postings,
                &owner,
                owner.owner_path.clone(),
                TopologyNodeKindV1::Owner,
                None,
                topology_navigation_features(&owner.owner_path),
            );
            for node in &owner.nodes {
                node_count = node_count.saturating_add(1);
                parser_native_node_count = parser_native_node_count.saturating_add(1);
                add_postings(
                    &mut postings,
                    &owner,
                    node.structural_selector.clone(),
                    node.kind.clone(),
                    Some(node.structural_selector.clone()),
                    node.features
                        .iter()
                        .flat_map(|key| topology_feature_terms(key)),
                );
            }
        }
        Ok(Self {
            postings: postings
                .into_iter()
                .map(|(feature, mut hits)| {
                    hits.sort_by(hit_order);
                    hits.dedup_by(|left, right| {
                        left.owner_path == right.owner_path
                            && left.topology_locator == right.topology_locator
                    });
                    (feature, Arc::from(hits))
                })
                .collect(),
            owner_shard_digests,
            node_count,
            parser_native_node_count,
        })
    }

    /// Intersect normalized features, beginning with the shortest posting.
    #[must_use]
    pub fn query(&self, query: &str, limit: usize) -> Vec<TopologyHitV1> {
        let features = topology_feature_terms(query)
            .into_iter()
            .collect::<BTreeSet<_>>();
        if features.is_empty() || limit == 0 {
            return Vec::new();
        }
        let Some(mut posting_lists) = features
            .iter()
            .map(|feature| {
                self.postings
                    .get(feature)
                    .map(|postings| (feature, postings))
            })
            .collect::<Option<Vec<_>>>()
        else {
            return Vec::new();
        };
        posting_lists.sort_by_key(|(_, postings)| postings.len());
        let mut result = posting_lists
            .first()
            .into_iter()
            .flat_map(|(_, postings)| postings.iter().cloned())
            .collect::<Vec<_>>();
        for (_, postings) in posting_lists.iter().skip(1) {
            result.retain(|candidate| {
                postings
                    .binary_search_by(|posting| hit_order(posting, candidate))
                    .is_ok()
            });
            if result.is_empty() {
                return result;
            }
        }
        let matched_features = features.into_iter().collect::<Vec<_>>();
        result.truncate(limit);
        for hit in &mut result {
            hit.matched_features.clone_from(&matched_features);
        }
        result
    }

    #[must_use]
    pub fn owner_shard_digest(&self, owner_path: &str) -> Option<&str> {
        self.owner_shard_digests.get(owner_path).map(String::as_str)
    }

    #[must_use]
    pub const fn node_count(&self) -> usize {
        self.node_count
    }

    #[must_use]
    pub const fn parser_native_node_count(&self) -> usize {
        self.parser_native_node_count
    }
}

/// Intersect content-bound parser-native hits with owner and language scope.
pub fn ranked_text_topology_selector_carrier(
    owner_scope: &BTreeSet<String>,
    admitted_languages: &BTreeSet<String>,
    hits: impl IntoIterator<Item = TopologyHitV1>,
    selector_limit: usize,
) -> Result<Vec<RankedTextTopologySelectorHitV1>, String> {
    let mut candidates = BTreeMap::new();
    for hit in hits {
        let TopologyNodeKindV1::ParserNative { language_id, .. } = &hit.kind else {
            continue;
        };
        let Some(selector) = hit.structural_selector else {
            continue;
        };
        if !owner_scope.contains(&hit.owner_path) || !admitted_languages.contains(language_id) {
            continue;
        }
        candidates
            .entry(selector.clone())
            .or_insert(RankedTextTopologySelectorHitV1 {
                owner_path: hit.owner_path,
                structural_selector: selector,
            });
        if candidates.len() > selector_limit {
            return Err(format!(
                "query-not-ready: Tantivy topology selector carrier budget exceeded: candidates={} limit={selector_limit}",
                candidates.len()
            ));
        }
    }
    Ok(candidates.into_values().collect())
}

fn validate_owner(owner: &TopologyOwnerV1) -> Result<(), String> {
    validate_owner_path(&owner.owner_path)?;
    validate_digest(&owner.owner_content_digest)?;
    let mut selectors = BTreeSet::new();
    for node in &owner.nodes {
        if node.features.is_empty() || !selectors.insert(node.structural_selector.as_str()) {
            return Err("topology node features are empty or selector is duplicated".to_owned());
        }
        let selector = CanonicalItemSelector::parse_root_or_exact_descendant(
            node.structural_selector.clone(),
        )?;
        if selector.owner_path()? != owner.owner_path {
            return Err("topology node selector owner does not match its shard".to_owned());
        }
        let TopologyNodeKindV1::ParserNative {
            language_id,
            native_kind,
        } = &node.kind
        else {
            return Err("provider topology input may contain only parser-native nodes".to_owned());
        };
        if selector.language_id.as_str() != language_id || selector.kind.as_str() != native_kind {
            return Err("topology node kind does not match its canonical selector".to_owned());
        }
    }
    Ok(())
}

fn validate_owner_path(owner_path: &str) -> Result<(), String> {
    if owner_path.is_empty()
        || owner_path.starts_with('/')
        || owner_path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == ".." || part.contains('\\'))
    {
        return Err("topology owner path is invalid".to_owned());
    }
    Ok(())
}

fn validate_digest(digest: &str) -> Result<(), String> {
    let valid = digest.strip_prefix("blake3-256:").is_some_and(|payload| {
        payload.len() == 64
            && payload
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    });
    if valid {
        Ok(())
    } else {
        Err("topology owner content digest is invalid".to_owned())
    }
}

fn owner_directories(owner_path: &str) -> Vec<String> {
    let parts = owner_path.split('/').collect::<Vec<_>>();
    (1..parts.len()).map(|end| parts[..end].join("/")).collect()
}

fn add_postings(
    postings: &mut BTreeMap<String, Vec<TopologyHitV1>>,
    owner: &TopologyOwnerV1,
    topology_locator: String,
    kind: TopologyNodeKindV1,
    structural_selector: Option<String>,
    features: impl IntoIterator<Item = String>,
) {
    for feature in features.into_iter().collect::<BTreeSet<_>>() {
        postings
            .entry(feature.clone())
            .or_default()
            .push(TopologyHitV1 {
                owner_path: owner.owner_path.clone(),
                owner_content_digest: owner.owner_content_digest.clone(),
                topology_locator: topology_locator.clone(),
                kind: kind.clone(),
                structural_selector: structural_selector.clone(),
                matched_features: vec![feature],
            });
    }
}

fn hit_order(left: &TopologyHitV1, right: &TopologyHitV1) -> std::cmp::Ordering {
    left.owner_path
        .cmp(&right.owner_path)
        .then_with(|| left.topology_locator.cmp(&right.topology_locator))
        .then_with(|| left.kind.cmp(&right.kind))
}

/// Normalize a parser-owned node feature into the shared V1 vocabulary.
#[must_use]
pub fn topology_feature_terms(value: &str) -> Vec<String> {
    let mut terms = BTreeSet::new();
    let mut current = String::new();
    for character in value.chars() {
        if character.is_alphanumeric() || character == '_' || character == '-' {
            current.push(character);
        } else if !current.is_empty() {
            insert_term_parts(&mut terms, &current);
            current.clear();
        }
    }
    if !current.is_empty() {
        insert_term_parts(&mut terms, &current);
    }
    terms.into_iter().collect()
}

/// Normalize one owner path into repository-navigation features.
#[must_use]
pub fn topology_navigation_features(owner_path: &str) -> Vec<String> {
    let mut terms = topology_feature_terms(owner_path);
    terms.push(owner_path.to_ascii_lowercase());
    terms.sort_unstable();
    terms.dedup();
    terms
}

fn insert_term_parts(terms: &mut BTreeSet<String>, term: &str) {
    terms.insert(term.to_lowercase());
    for part in term.split(['_', '-']).filter(|part| !part.is_empty()) {
        if part.chars().count() >= 2 {
            terms.insert(part.to_lowercase());
        }
        let characters = part.chars().collect::<Vec<_>>();
        let mut start = 0;
        for index in 1..characters.len() {
            let previous = characters[index - 1];
            let current = characters[index];
            let next = characters.get(index + 1).copied();
            let boundary = (previous.is_lowercase() || previous.is_numeric())
                && current.is_uppercase()
                || previous.is_uppercase()
                    && current.is_uppercase()
                    && next.is_some_and(char::is_lowercase);
            if boundary {
                insert_camel_part(terms, &characters[start..index]);
                start = index;
            }
        }
        insert_camel_part(terms, &characters[start..]);
    }
}

fn insert_camel_part(terms: &mut BTreeSet<String>, characters: &[char]) {
    if characters.len() >= 2 {
        terms.insert(characters.iter().collect::<String>().to_lowercase());
    }
}

fn topology_shard_digest(owner: &TopologyOwnerV1) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"agent.semantic-protocols.topology-index-owner-shard.v1\0");
    hasher.update(owner.owner_path.as_bytes());
    hasher.update(&[0]);
    hasher.update(owner.owner_content_digest.as_bytes());
    for node in &owner.nodes {
        hasher.update(&[0]);
        hasher.update(node.structural_selector.as_bytes());
        if let TopologyNodeKindV1::ParserNative {
            language_id,
            native_kind,
        } = &node.kind
        {
            hasher.update(&[0]);
            hasher.update(language_id.as_bytes());
            hasher.update(&[0]);
            hasher.update(native_kind.as_bytes());
        }
        for feature in &node.features {
            hasher.update(&[0]);
            hasher.update(feature.as_bytes());
        }
    }
    format!("blake3-256:{}", hasher.finalize().to_hex())
}

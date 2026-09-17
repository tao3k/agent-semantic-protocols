// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Language-neutral P0 Symbol Skeleton Index implementation.
//!
//! Inputs are owner paths and parser-owned symbol keys. Source/function bodies
//! are not representable in this crate's input types.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

#[derive(Clone, Debug, Eq, PartialEq)]
/// One parser-owned symbol and its normalized lookup keys.
pub struct SymbolSkeletonRecordV1 {
    /// Canonical language-provider selector returned to exact Query.
    pub structural_selector: String,
    /// Parser-produced names used only for symbol lookup.
    pub keys: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// One content-bound owner shard supplied by any language provider.
pub struct SymbolSkeletonOwnerV1 {
    /// Workspace-relative owner path.
    pub owner_path: String,
    /// Canonical lowercase BLAKE3 identity of the exact owner bytes.
    pub owner_content_digest: String,
    /// Provider language identity when the owner is parser-backed.
    pub language_id: Option<String>,
    /// Parser-produced symbol skeletons; function bodies are not representable.
    pub symbols: Vec<SymbolSkeletonRecordV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Compact resident lookup result linking navigation to an exact selector.
pub struct SymbolSkeletonHitV1 {
    /// Workspace-relative owner path.
    pub owner_path: String,
    /// Content identity against which the hit was indexed.
    pub owner_content_digest: String,
    /// Provider language identity, when known.
    pub language_id: Option<String>,
    /// Exact parser selector, or `None` for a path-navigation hit.
    pub structural_selector: Option<String>,
    /// Normalized conjunction terms satisfied by this hit.
    pub matched_keys: Vec<String>,
}

#[derive(Debug, Default)]
/// Immutable language-neutral postings index over paths and parser symbols.
pub struct SymbolSkeletonIndexV1 {
    postings: BTreeMap<String, Arc<[SymbolSkeletonHitV1]>>,
    owner_shard_digests: BTreeMap<String, String>,
    symbol_count: usize,
}

impl SymbolSkeletonIndexV1 {
    /// Build deterministic owner shards and rarest-first postings.
    ///
    /// # Errors
    ///
    /// Rejects malformed paths or digests, empty/duplicate symbol records, and
    /// duplicate owner paths.
    pub fn build(owners: impl IntoIterator<Item = SymbolSkeletonOwnerV1>) -> Result<Self, String> {
        let mut postings = BTreeMap::<String, Vec<SymbolSkeletonHitV1>>::new();
        let mut owner_shard_digests = BTreeMap::new();
        let mut symbol_count = 0usize;
        for mut owner in owners {
            validate_owner(&owner)?;
            for symbol in &mut owner.symbols {
                symbol.keys.sort_unstable();
                symbol.keys.dedup();
            }
            owner
                .symbols
                .sort_by(|left, right| left.structural_selector.cmp(&right.structural_selector));
            let shard_digest = symbol_shard_digest(&owner);
            if owner_shard_digests
                .insert(owner.owner_path.clone(), shard_digest)
                .is_some()
            {
                return Err("symbol skeleton contains a duplicate owner path".to_owned());
            }
            add_postings(
                &mut postings,
                &owner,
                None,
                owner_navigation_keys(&owner.owner_path),
            );
            for symbol in &owner.symbols {
                symbol_count = symbol_count.saturating_add(1);
                add_postings(
                    &mut postings,
                    &owner,
                    Some(symbol.structural_selector.clone()),
                    symbol
                        .keys
                        .iter()
                        .flat_map(|key| symbol_skeleton_terms(key)),
                );
            }
        }
        Ok(Self {
            postings: postings
                .into_iter()
                .map(|(term, mut hits)| {
                    hits.sort_by(hit_order);
                    hits.dedup_by(|left, right| {
                        left.owner_path == right.owner_path
                            && left.structural_selector == right.structural_selector
                    });
                    (term, Arc::from(hits))
                })
                .collect(),
            owner_shard_digests,
            symbol_count,
        })
    }

    /// Intersect normalized query terms, beginning with the shortest posting.
    #[must_use]
    pub fn query(&self, query: &str, limit: usize) -> Vec<SymbolSkeletonHitV1> {
        let terms = symbol_skeleton_terms(query)
            .into_iter()
            .collect::<BTreeSet<_>>();
        if terms.is_empty() || limit == 0 {
            return Vec::new();
        }
        let Some(mut posting_lists) = terms
            .iter()
            .map(|term| self.postings.get(term).map(|postings| (term, postings)))
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
        let matched_keys = terms.into_iter().collect::<Vec<_>>();
        result.truncate(limit);
        for hit in &mut result {
            hit.matched_keys.clone_from(&matched_keys);
        }
        result
    }

    #[must_use]
    /// Return the content-bound digest of one immutable owner shard.
    pub fn owner_shard_digest(&self, owner_path: &str) -> Option<&str> {
        self.owner_shard_digests.get(owner_path).map(String::as_str)
    }

    #[must_use]
    /// Return the number of parser symbols, excluding path-navigation rows.
    pub const fn symbol_count(&self) -> usize {
        self.symbol_count
    }
}

fn validate_owner(owner: &SymbolSkeletonOwnerV1) -> Result<(), String> {
    let digest_payload = owner
        .owner_content_digest
        .strip_prefix("blake3-256:")
        .filter(|payload| {
            payload.len() == 64
                && payload
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        });
    if owner.owner_path.is_empty()
        || owner.owner_path.starts_with('/')
        || owner
            .owner_path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        || digest_payload.is_none()
    {
        return Err("symbol skeleton owner identity is invalid".to_owned());
    }
    let mut selectors = BTreeSet::new();
    for symbol in &owner.symbols {
        if symbol.structural_selector.is_empty()
            || symbol.keys.is_empty()
            || !selectors.insert(symbol.structural_selector.as_str())
        {
            return Err("symbol skeleton record is empty or duplicated".to_owned());
        }
    }
    Ok(())
}

fn add_postings(
    postings: &mut BTreeMap<String, Vec<SymbolSkeletonHitV1>>,
    owner: &SymbolSkeletonOwnerV1,
    structural_selector: Option<String>,
    terms: impl IntoIterator<Item = String>,
) {
    for term in terms.into_iter().collect::<BTreeSet<_>>() {
        postings
            .entry(term.clone())
            .or_default()
            .push(SymbolSkeletonHitV1 {
                owner_path: owner.owner_path.clone(),
                owner_content_digest: owner.owner_content_digest.clone(),
                language_id: owner.language_id.clone(),
                structural_selector: structural_selector.clone(),
                matched_keys: vec![term],
            });
    }
}

fn hit_order(left: &SymbolSkeletonHitV1, right: &SymbolSkeletonHitV1) -> std::cmp::Ordering {
    left.owner_path
        .cmp(&right.owner_path)
        .then_with(|| left.structural_selector.cmp(&right.structural_selector))
}

/// Normalize one parser-owned symbol/path value into the shared V1 lookup
/// vocabulary. Snake/kebab components and CamelCase components are retained
/// alongside the complete normalized identifier.
#[must_use]
pub fn symbol_skeleton_terms(value: &str) -> Vec<String> {
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

/// Normalize one workspace-relative owner path into shallow navigation keys.
#[must_use]
pub fn symbol_skeleton_navigation_keys(owner_path: &str) -> Vec<String> {
    let mut terms = symbol_skeleton_terms(owner_path);
    terms.push(owner_path.to_ascii_lowercase());
    terms.sort_unstable();
    terms.dedup();
    terms
}

fn owner_navigation_keys(owner_path: &str) -> Vec<String> {
    symbol_skeleton_navigation_keys(owner_path)
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

fn symbol_shard_digest(owner: &SymbolSkeletonOwnerV1) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"agent.semantic-protocols.symbol-skeleton-shard.v1\0");
    hasher.update(owner.owner_path.as_bytes());
    hasher.update(&[0]);
    hasher.update(owner.owner_content_digest.as_bytes());
    hasher.update(&[0]);
    if let Some(language_id) = &owner.language_id {
        hasher.update(language_id.as_bytes());
    }
    for symbol in &owner.symbols {
        hasher.update(&[0]);
        hasher.update(symbol.structural_selector.as_bytes());
        for key in &symbol.keys {
            hasher.update(&[0]);
            hasher.update(key.as_bytes());
        }
    }
    format!("blake3-256:{}", hasher.finalize().to_hex())
}

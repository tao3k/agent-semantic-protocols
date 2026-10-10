// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Content-addressed lexical generation planning.
//!
//! File discovery, repository admission, lexical fact extraction, and Tantivy
//! indexing are separate authorities. This module joins their immutable
//! receipts without walking the filesystem or launching tools.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use serde::Deserialize;
use serde::Serialize;

pub const LEXICAL_GENERATION_PLAN_SCHEMA_ID: &str =
    "agent.semantic-protocols.lexical-generation-plan";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdmittedLexicalOwner<'a> {
    pub owner_path: &'a str,
    pub content_digest: &'a str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LexicalOwnerFact<'a> {
    pub owner_path: &'a str,
    pub content_digest: &'a str,
    pub query_keys: &'a [String],
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LexicalShardArtifact {
    pub shard_key: String,
    pub artifact_digest: String,
    pub owner_path: String,
    pub content_digest: String,
    pub analyzer_digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LexicalShardDisposition {
    Reuse,
    Rebuild,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LexicalShardPlanEntry {
    pub owner_path: String,
    pub content_digest: String,
    pub shard_key: String,
    pub disposition: LexicalShardDisposition,
    pub prior_artifact_digest: Option<String>,
    pub query_keys: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LexicalGenerationPlan {
    pub schema_id: String,
    pub schema_version: String,
    pub analyzer_digest: String,
    pub inventory_digest: String,
    pub admitted_owner_digest: String,
    pub lexical_fact_digest: String,
    pub entries: Vec<LexicalShardPlanEntry>,
    pub retired_artifact_digests: Vec<String>,
    pub plan_digest: String,
}

impl LexicalGenerationPlan {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != LEXICAL_GENERATION_PLAN_SCHEMA_ID || self.schema_version != "1" {
            return Err("lexical generation plan schema mismatch".to_owned());
        }
        validate_digest("analyzerDigest", &self.analyzer_digest)?;
        validate_digest("inventoryDigest", &self.inventory_digest)?;
        validate_digest("admittedOwnerDigest", &self.admitted_owner_digest)?;
        validate_digest("lexicalFactDigest", &self.lexical_fact_digest)?;
        validate_digest("planDigest", &self.plan_digest)?;
        if self
            .entries
            .windows(2)
            .any(|entries| entries[0].owner_path >= entries[1].owner_path)
        {
            return Err("lexical shard entries must be unique and path-sorted".to_owned());
        }
        for entry in &self.entries {
            validate_digest("contentDigest", &entry.content_digest)?;
            validate_digest("shardKey", &entry.shard_key)?;
            if entry.owner_path.trim().is_empty()
                || entry.query_keys.is_empty()
                || entry.query_keys.windows(2).any(|keys| keys[0] >= keys[1])
            {
                return Err("lexical shard entry is incomplete or non-canonical".to_owned());
            }
            match (&entry.disposition, &entry.prior_artifact_digest) {
                (LexicalShardDisposition::Reuse, Some(digest)) => {
                    validate_digest("priorArtifactDigest", digest)?;
                }
                (LexicalShardDisposition::Rebuild, None) => {}
                _ => return Err("lexical shard disposition and prior artifact disagree".to_owned()),
            }
        }
        for digest in &self.retired_artifact_digests {
            validate_digest("retiredArtifactDigest", digest)?;
        }
        if self
            .retired_artifact_digests
            .windows(2)
            .any(|digests| digests[0] >= digests[1])
        {
            return Err("retired lexical artifacts must be unique and sorted".to_owned());
        }
        let expected = digest_json(
            "lexical generation plan",
            &(
                self.analyzer_digest.as_str(),
                &self.inventory_digest,
                &self.admitted_owner_digest,
                &self.lexical_fact_digest,
                &self.entries,
                &self.retired_artifact_digests,
            ),
        )?;
        if self.plan_digest != expected {
            return Err("lexical generation plan digest drift".to_owned());
        }
        Ok(())
    }

    #[must_use]
    pub fn reused_shard_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.disposition == LexicalShardDisposition::Reuse)
            .count()
    }

    #[must_use]
    pub fn rebuilt_shard_count(&self) -> usize {
        self.entries.len() - self.reused_shard_count()
    }
}

/// Join `fd` inventory, repository admission, native lexical facts, and prior
/// Tantivy shard artifacts into one deterministic generation build plan.
///
/// This function cannot discover files, read source, or infer lexical facts.
/// Every admitted owner must occur in the inventory and have one content-bound
/// provider/native-parser fact before any reusable shard is attached. Ripgrep
/// is the cold query executor and is deliberately absent from this build plan.
pub fn plan_lexical_generation<'a>(
    analyzer_digest: &str,
    fd_inventory: impl IntoIterator<Item = &'a str>,
    admitted_owners: impl IntoIterator<Item = AdmittedLexicalOwner<'a>>,
    lexical_facts: impl IntoIterator<Item = LexicalOwnerFact<'a>>,
    prior_artifacts: impl IntoIterator<Item = LexicalShardArtifact>,
) -> Result<LexicalGenerationPlan, String> {
    validate_digest("analyzerDigest", analyzer_digest)?;
    let inventory = canonical_inventory(fd_inventory)?;
    let admitted = canonical_admitted_owners(admitted_owners)?;
    for owner_path in admitted.keys() {
        if !inventory.contains(owner_path) {
            return Err(format!(
                "repository-admitted owner is absent from fd inventory: {owner_path}"
            ));
        }
    }

    let facts = canonical_lexical_facts(lexical_facts)?;
    if facts.len() != admitted.len() {
        return Err("lexical facts do not cover the complete admitted owner set".to_owned());
    }
    for (owner_path, content_digest) in &admitted {
        let fact = facts
            .get(owner_path)
            .ok_or_else(|| format!("lexical facts omitted admitted owner: {owner_path}"))?;
        if fact.content_digest != *content_digest {
            return Err(format!("lexical fact content digest drift: {owner_path}"));
        }
    }
    if facts
        .keys()
        .any(|owner_path| !admitted.contains_key(owner_path))
    {
        return Err("lexical facts contain an unadmitted owner".to_owned());
    }

    let prior = canonical_prior_artifacts(prior_artifacts)?;
    let mut entries = Vec::with_capacity(admitted.len());
    let mut attached_prior_keys = BTreeSet::new();
    for (owner_path, content_digest) in &admitted {
        let fact = &facts[owner_path];
        let shard_key = lexical_shard_key(
            analyzer_digest,
            owner_path,
            content_digest,
            &fact.query_keys,
        )?;
        let reusable = prior.get(&shard_key).filter(|artifact| {
            artifact.owner_path == *owner_path
                && artifact.content_digest == *content_digest
                && artifact.analyzer_digest == analyzer_digest
        });
        if let Some(artifact) = reusable {
            attached_prior_keys.insert(shard_key.clone());
            entries.push(LexicalShardPlanEntry {
                owner_path: owner_path.clone(),
                content_digest: content_digest.clone(),
                shard_key,
                disposition: LexicalShardDisposition::Reuse,
                prior_artifact_digest: Some(artifact.artifact_digest.clone()),
                query_keys: fact.query_keys.clone(),
            });
        } else {
            entries.push(LexicalShardPlanEntry {
                owner_path: owner_path.clone(),
                content_digest: content_digest.clone(),
                shard_key,
                disposition: LexicalShardDisposition::Rebuild,
                prior_artifact_digest: None,
                query_keys: fact.query_keys.clone(),
            });
        }
    }

    let mut retired_artifact_digests = prior
        .iter()
        .filter(|(shard_key, _)| !attached_prior_keys.contains(*shard_key))
        .map(|(_, artifact)| artifact.artifact_digest.clone())
        .collect::<Vec<_>>();
    retired_artifact_digests.sort_unstable();
    retired_artifact_digests.dedup();

    let inventory_digest = digest_json("fd inventory", &inventory)?;
    let admitted_owner_digest = digest_json("admitted owners", &admitted)?;
    let lexical_fact_digest = digest_json("lexical owner facts", &facts)?;
    let plan_digest = digest_json(
        "lexical generation plan",
        &(
            analyzer_digest,
            &inventory_digest,
            &admitted_owner_digest,
            &lexical_fact_digest,
            &entries,
            &retired_artifact_digests,
        ),
    )?;
    let plan = LexicalGenerationPlan {
        schema_id: LEXICAL_GENERATION_PLAN_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        analyzer_digest: analyzer_digest.to_owned(),
        inventory_digest,
        admitted_owner_digest,
        lexical_fact_digest,
        entries,
        retired_artifact_digests,
        plan_digest,
    };
    plan.validate()?;
    Ok(plan)
}

fn canonical_inventory<'a>(
    inventory: impl IntoIterator<Item = &'a str>,
) -> Result<BTreeSet<String>, String> {
    let mut canonical = BTreeSet::new();
    for owner_path in inventory {
        if owner_path.trim().is_empty() || !canonical.insert(owner_path.to_owned()) {
            return Err("fd inventory contains an empty or duplicate path".to_owned());
        }
    }
    Ok(canonical)
}

fn canonical_admitted_owners<'a>(
    owners: impl IntoIterator<Item = AdmittedLexicalOwner<'a>>,
) -> Result<BTreeMap<String, String>, String> {
    let mut canonical = BTreeMap::new();
    for owner in owners {
        validate_digest("contentDigest", owner.content_digest)?;
        if owner.owner_path.trim().is_empty()
            || canonical
                .insert(owner.owner_path.to_owned(), owner.content_digest.to_owned())
                .is_some()
        {
            return Err("repository admission contains an empty or duplicate owner".to_owned());
        }
    }
    Ok(canonical)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct CanonicalLexicalOwnerFact {
    content_digest: String,
    query_keys: Vec<String>,
}

fn canonical_lexical_facts<'a>(
    facts: impl IntoIterator<Item = LexicalOwnerFact<'a>>,
) -> Result<BTreeMap<String, CanonicalLexicalOwnerFact>, String> {
    let mut canonical = BTreeMap::new();
    for fact in facts {
        validate_digest("contentDigest", fact.content_digest)?;
        let mut query_keys = fact.query_keys.to_vec();
        query_keys.sort_unstable();
        query_keys.dedup();
        if fact.owner_path.trim().is_empty()
            || query_keys.is_empty()
            || query_keys.iter().any(|key| key.trim().is_empty())
            || canonical
                .insert(
                    fact.owner_path.to_owned(),
                    CanonicalLexicalOwnerFact {
                        content_digest: fact.content_digest.to_owned(),
                        query_keys,
                    },
                )
                .is_some()
        {
            return Err("lexical facts contain an incomplete or duplicate owner".to_owned());
        }
    }
    Ok(canonical)
}

fn canonical_prior_artifacts(
    artifacts: impl IntoIterator<Item = LexicalShardArtifact>,
) -> Result<BTreeMap<String, LexicalShardArtifact>, String> {
    let mut canonical = BTreeMap::new();
    for artifact in artifacts {
        validate_digest("shardKey", &artifact.shard_key)?;
        validate_digest("artifactDigest", &artifact.artifact_digest)?;
        validate_digest("contentDigest", &artifact.content_digest)?;
        validate_digest("analyzerDigest", &artifact.analyzer_digest)?;
        if artifact.owner_path.trim().is_empty()
            || canonical
                .insert(artifact.shard_key.clone(), artifact)
                .is_some()
        {
            return Err(
                "prior lexical artifacts contain an incomplete or duplicate shard".to_owned(),
            );
        }
    }
    Ok(canonical)
}

fn lexical_shard_key(
    analyzer_digest: &str,
    owner_path: &str,
    content_digest: &str,
    query_keys: &[String],
) -> Result<String, String> {
    digest_json(
        "lexical shard key",
        &(analyzer_digest, owner_path, content_digest, query_keys),
    )
}

fn digest_json(label: &str, value: &impl Serialize) -> Result<String, String> {
    let bytes = serde_json::to_vec(value).map_err(|error| format!("encode {label}: {error}"))?;
    Ok(format!("blake3-256:{}", blake3::hash(&bytes).to_hex()))
}

fn validate_digest(field: &str, digest: &str) -> Result<(), String> {
    match crate::canonical_blake3_digest(digest) {
        Ok(canonical) if canonical == digest => Ok(()),
        _ => Err(format!("{field} is not a canonical BLAKE3 digest")),
    }
}

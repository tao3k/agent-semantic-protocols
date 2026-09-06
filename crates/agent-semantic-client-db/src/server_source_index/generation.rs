// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Atomically publish one complete, ASP Server-owned source-index generation.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::ClientDbSourceIndexScopeFile as SourceIndexScopeFile;
use agent_semantic_content_identity::exact_selector_generation_fixture::{
    ExactSelectorGenerationIdentityV1, fixture_digest_v1,
};
use agent_semantic_content_identity::workspace_search_identity::WorkspaceSearchIdentityV1;
use agent_semantic_search::exact_selector_fixture_publication::build_exact_selector_fixture_from_projection_records_v1;
use agent_semantic_search::exact_selector_generation_fixture::{
    ExactSelectorFixturePublicationV1, ExactSelectorGenerationMemorySearchV1,
    publish_immutable_exact_selector_generation_fixture_v1,
};

use crate::server_source_index::CurrentSourceIndexSnapshot;

static GENERATION_TRANSACTION_ID: AtomicU64 = AtomicU64::new(0);

/// Typed evidence required to publish one complete source-index generation.
pub struct WorkspaceSearchGenerationPublicationRequestV1<'a> {
    pub collection_scope: crate::server_source_index::collect::SourceIndexCollectionScope,
    pub snapshot: &'a CurrentSourceIndexSnapshot,
    pub files: &'a [SourceIndexScopeFile],
    pub registry_evidence: &'a agent_semantic_client_core::RuntimeProviderProjectionEvidence,
    pub artifact_root: &'a Path,
    pub project_root: &'a Path,
    pub provider_id: &'a str,
    pub source_extensions: &'a [String],
    pub workspace_identity: &'a WorkspaceSearchIdentityV1,
    pub generation_identity: ExactSelectorGenerationIdentityV1,
}

/// Paths and content identities committed by one generation transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishedSourceIndexGenerationV1 {
    pub exact_selector_fixture_publication:
        agent_semantic_search::exact_selector_fixture_publication::ExactSelectorFixturePublicationReceiptV1,
    pub generation_directory: PathBuf,
    pub provider_envelope_path: PathBuf,
    pub exact_selector_fixture_path: PathBuf,
    pub provider_relation_path: PathBuf,
    pub generation_digest: [u8; 32],
    pub fixture_digest: [u8; 32],
    pub provider_relation_digest: [u8; 32],
    pub workspace_identity_digest: [u8; 32],
}

/// Validate and atomically publish one immutable complete workspace search generation.
pub fn publish_workspace_search_generation_v1(
    publication: WorkspaceSearchGenerationPublicationRequestV1<'_>,
) -> Result<PublishedSourceIndexGenerationV1, String> {
    let crate::server_source_index::collect::SourceIndexCollectionScope::CompleteGeneration =
        &publication.collection_scope;
    publish_complete_workspace_search_generation_v1(publication)
}

fn publish_complete_workspace_search_generation_v1(
    publication: WorkspaceSearchGenerationPublicationRequestV1<'_>,
) -> Result<PublishedSourceIndexGenerationV1, String> {
    let _registry_evidence = publication.registry_evidence;
    let provider_files = publication
        .files
        .iter()
        .filter(|file| file.provider_id.as_str() == publication.provider_id)
        .collect::<Vec<_>>();
    if provider_files.is_empty() {
        return Err(format!(
            "complete generation has no provider owners: providerId={}",
            publication.provider_id
        ));
    }
    if provider_files.iter().any(|file| match file.projection_coverage {
        crate::ClientDbSourceIndexProjectionCoverage::Complete => {
            file.projection_diagnostic.is_some()
        }
        crate::ClientDbSourceIndexProjectionCoverage::SyntaxUnavailable => {
            file.projection_diagnostic.as_ref().is_none_or(|diagnostic| {
                diagnostic.reason_kind != agent_semantic_provider_transport::projection_batch::SOURCE_SYNTAX_UNAVAILABLE_REASON_KIND
                    || diagnostic.message.trim().is_empty()
                    || !file.selector_receipts.is_empty()
                    || !file.relations.is_empty()
            })
        }
        crate::ClientDbSourceIndexProjectionCoverage::NotDeclared => true,
    }) {
        return Err(format!(
            "complete generation owner is missing its provider projection or diagnostic receipt: providerId={}",
            publication.provider_id
        ));
    }
    let projection_records = provider_files
        .iter()
        .flat_map(|file| file.selector_receipts.iter())
        .map(|selector| selector.projection_record.clone())
        .collect::<Vec<_>>();
    let proofs_are_from_one_generation = projection_records.iter().all(|record| {
        let proof = &record.proof;
        proof.validate_shape().is_ok()
            && digest_matches_bytes(
                proof.workspace_root_digest().as_str(),
                &publication.generation_identity.workspace_root_digest,
            )
            && proof.workspace_root_digest().as_str()
                == blake3::Hash::from_bytes(
                    *publication.workspace_identity.source_snapshot_root_digest(),
                )
                .to_hex()
                .as_str()
            && digest_matches_bytes(
                proof.parser_identity_digest().as_str(),
                &publication.generation_identity.parser_identity_digest,
            )
            && digest_matches_bytes(
                proof.query_pack_digest().as_str(),
                &publication.generation_identity.query_pack_digest,
            )
    });
    if !proofs_are_from_one_generation {
        return Err(format!(
            "selector materialization proofs cross generation identity: providerId={}",
            publication.provider_id
        ));
    }
    let covered_owner_paths = projection_records
        .iter()
        .map(|record| record.proof.owner_path())
        .collect::<BTreeSet<_>>();
    let provider_owner_paths = provider_files
        .iter()
        .map(|file| {
            let path = file
                .path
                .strip_prefix(publication.project_root)
                .unwrap_or(file.path.as_path());
            path.to_string_lossy().replace('\\', "/")
        })
        .collect::<BTreeSet<_>>();
    if !covered_owner_paths
        .iter()
        .all(|path| provider_owner_paths.contains(*path))
    {
        return Err(format!(
            "provider projection contains an owner outside the admitted source envelope: providerId={} ownerCount={} projectedNonEmptyOwnerCount={}",
            publication.provider_id,
            provider_owner_paths.len(),
            covered_owner_paths.len()
        ));
    }
    if publication.workspace_identity.owner_count()
        != u32::try_from(provider_owner_paths.len()).unwrap_or(u32::MAX)
        || publication.workspace_identity.selector_count()
            != u32::try_from(projection_records.len()).unwrap_or(u32::MAX)
    {
        return Err(format!(
            "complete generation coverage does not match admitted workspace identity: providerId={}",
            publication.provider_id
        ));
    }

    let generation_digest = publication.generation_identity.generation_digest;
    let generation_digest_hex = generation_digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let mut relations = provider_files
        .iter()
        .flat_map(|file| file.relations.iter().cloned())
        .collect::<Vec<_>>();
    relations.sort();
    relations.dedup();
    let relation_generation = agent_semantic_content_identity::provider_projection_relation::ProviderRelationGeneration {
        schema_id: agent_semantic_content_identity::provider_projection_relation::PROVIDER_RELATION_GENERATION_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        generation_digest: format!("blake3-256:{generation_digest_hex}"),
        relations,
    };
    relation_generation.validate()?;
    let relation_bytes = serde_json::to_vec(&relation_generation)
        .map_err(|error| format!("failed to encode provider relation generation: {error}"))?;
    let provider_relation_digest = *blake3::hash(&relation_bytes).as_bytes();

    let fixture = build_exact_selector_fixture_from_projection_records_v1(
        &publication.generation_identity,
        projection_records.clone(),
    )
    .map_err(|error| format!("failed to build complete exact-selector generation: {error}"))?;
    let fixture_digest = *fixture_digest_v1(&fixture)
        .map_err(|error| format!("failed to identify exact-selector fixture: {error}"))?;
    let digest_hex = generation_digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let generations_root = publication
        .artifact_root
        .join("source-index-generations")
        .join("v1");
    fs::create_dir_all(&generations_root).map_err(|error| {
        format!(
            "failed to create source-index generation root {}: {error}",
            generations_root.display()
        )
    })?;
    let generation_directory = generations_root.join(&digest_hex);
    let staging_directory = generations_root.join(format!(
        ".{digest_hex}.tmp-{}-{}",
        std::process::id(),
        GENERATION_TRANSACTION_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&staging_directory).map_err(|error| {
        format!(
            "failed to create source-index generation transaction {}: {error}",
            staging_directory.display()
        )
    })?;
    let provider_envelope_path =
        crate::server_source_index::publish_provider_source_snapshot_envelope(
            crate::server_source_index::ProviderSourceSnapshotEnvelopePublicationV1 {
                snapshot: publication.snapshot,
                provider_id: publication.provider_id,
                address_provider_digest: &agent_semantic_artifacts::provider_digest(
                    publication.registry_evidence.fingerprint.as_bytes(),
                ),
                source_extensions: publication.source_extensions,
                artifact_root: &staging_directory,
                provider_workspace_root: publication.project_root,
            },
        )
        .inspect_err(|_| {
            let _ = fs::remove_dir_all(&staging_directory);
        })?;
    let exact_selector_generation_directory = staging_directory.join("exact-selector");
    let exact_selector_fixture_path =
        publish_immutable_exact_selector_generation_fixture_v1(ExactSelectorFixturePublicationV1 {
            generation_directory: &exact_selector_generation_directory,
            fixture: &fixture,
            workspace_identity: publication.workspace_identity,
            generation_digest,
            fixture_digest,
        })
        .map_err(|error| {
            let _ = fs::remove_dir_all(&staging_directory);
            format!("failed to stage exact-selector generation: {error:?}")
        })?;
    let provider_relation_path = staging_directory.join("provider-relations.v1.json");
    fs::write(&provider_relation_path, &relation_bytes).map_err(|error| {
        let _ = fs::remove_dir_all(&staging_directory);
        format!("failed to stage provider relation generation: {error}")
    })?;
    fs::File::open(&provider_envelope_path)
        .and_then(|file| file.sync_all())
        .map_err(|error| {
            let _ = fs::remove_dir_all(&staging_directory);
            format!(
                "failed to sync provider source envelope {}: {error}",
                provider_envelope_path.display()
            )
        })?;
    fs::File::open(&provider_relation_path)
        .and_then(|file| file.sync_all())
        .map_err(|error| {
            let _ = fs::remove_dir_all(&staging_directory);
            format!(
                "failed to sync provider relation generation {}: {error}",
                provider_relation_path.display()
            )
        })?;
    fs::File::open(&staging_directory)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            let _ = fs::remove_dir_all(&staging_directory);
            format!(
                "failed to sync source-index generation transaction {}: {error}",
                staging_directory.display()
            )
        })?;

    let envelope_relative_path = provider_envelope_path
        .strip_prefix(&staging_directory)
        .map_err(|error| {
            let _ = fs::remove_dir_all(&staging_directory);
            format!("provider envelope escaped generation transaction: {error}")
        })?
        .to_path_buf();
    let fixture_relative_path = exact_selector_fixture_path
        .strip_prefix(&staging_directory)
        .map_err(|error| {
            let _ = fs::remove_dir_all(&staging_directory);
            format!("exact-selector fixture escaped generation transaction: {error}")
        })?
        .to_path_buf();
    let relation_relative_path = provider_relation_path
        .strip_prefix(&staging_directory)
        .map_err(|error| {
            let _ = fs::remove_dir_all(&staging_directory);
            format!("provider relation generation escaped transaction: {error}")
        })?
        .to_path_buf();
    if let Err(rename_error) = fs::rename(&staging_directory, &generation_directory) {
        let existing_fixture = generation_directory.join(&fixture_relative_path);
        let existing_envelope = generation_directory.join(&envelope_relative_path);
        let existing_relations = generation_directory.join(&relation_relative_path);
        let existing_relation_digest = fs::read(&existing_relations)
            .ok()
            .map(|bytes| *blake3::hash(&bytes).as_bytes());
        let concurrent_generation_is_complete = existing_envelope.is_file()
            && existing_relation_digest == Some(provider_relation_digest)
            && ExactSelectorGenerationMemorySearchV1::load_immutable_artifact(
                &existing_fixture,
                publication.workspace_identity,
                generation_digest,
                fixture_digest,
            )
            .is_ok();
        if !concurrent_generation_is_complete {
            let _ = fs::remove_dir_all(&staging_directory);
            return Err(format!(
                "failed to atomically publish complete source-index generation {}: {rename_error}",
                generation_directory.display()
            ));
        }
        fs::remove_dir_all(&staging_directory).map_err(|error| {
            format!(
                "failed to remove superseded source-index generation transaction {}: {error}",
                staging_directory.display()
            )
        })?;
    }
    fs::File::open(&generations_root)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            format!(
                "failed to sync source-index generation root {}: {error}",
                generations_root.display()
            )
        })?;
    Ok(PublishedSourceIndexGenerationV1 {
        exact_selector_fixture_publication:
            agent_semantic_search::exact_selector_fixture_publication::publish_exact_selector_fixture_v1(
                publication.artifact_root,
                &publication.generation_identity,
                projection_records,
            )?,
        provider_envelope_path: generation_directory.join(envelope_relative_path),
        exact_selector_fixture_path: generation_directory.join(fixture_relative_path),
        provider_relation_path: generation_directory.join(relation_relative_path),
        generation_directory,
        generation_digest,
        fixture_digest,
        provider_relation_digest,
        workspace_identity_digest: *publication.workspace_identity.identity_digest(),
    })
}

fn digest_matches_bytes(encoded: &str, expected: &[u8; 32]) -> bool {
    encoded == blake3::Hash::from_bytes(*expected).to_hex().as_str()
}

//! Atomically publish one complete, provider-owned source-index generation.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use agent_semantic_client_db::ClientDbSourceIndexScopeFile as SourceIndexScopeFile;
use agent_semantic_content_identity::exact_selector_generation_fixture::{
    ExactSelectorGenerationIdentityV1, fixture_digest_v1,
};
use agent_semantic_content_identity::workspace_search_identity::WorkspaceSearchIdentityV1;
use agent_semantic_search::exact_selector_fixture_publication::build_exact_selector_fixture_from_projection_records_v1;
use agent_semantic_search::exact_selector_generation_fixture::{
    ExactSelectorGenerationMemorySearchV1, publish_immutable_exact_selector_generation_fixture_v1,
};

use super::CurrentSourceIndexSnapshot;

static GENERATION_TRANSACTION_ID: AtomicU64 = AtomicU64::new(0);

/// Typed inputs for publishing one ordinary target-provider source envelope.
pub struct TargetProviderSourceEnvelopePublicationRequestV1<'a> {
    pub collection_scope: super::collect::SourceIndexCollectionScope,
    pub provider_registry: &'a agent_semantic_client_core::ProviderRegistrySnapshot,
    pub artifact_root: &'a Path,
    pub project_root: &'a Path,
}

/// Publish one provider-scoped source envelope without opening a complete
/// workspace generation transaction.
pub fn publish_target_provider_source_envelope_v1(
    publication: TargetProviderSourceEnvelopePublicationRequestV1<'_>,
) -> Result<PathBuf, String> {
    let requested_provider = match &publication.collection_scope {
        super::collect::SourceIndexCollectionScope::TargetProvider {
            language_id,
            provider_id,
        } => publication
            .provider_registry
            .providers
            .iter()
            .find(|provider| {
                &provider.language_id == language_id && &provider.provider_id == provider_id
            })
            .ok_or_else(|| {
                format!(
                    "requested target provider is not registered: languageId={} providerId={}",
                    language_id, provider_id
                )
            })?,
        super::collect::SourceIndexCollectionScope::TargetProviderId { provider_id } => {
            let mut matches = publication
                .provider_registry
                .providers
                .iter()
                .filter(|provider| &provider.provider_id == provider_id);
            let requested_provider = matches.next().ok_or_else(|| {
                format!("requested target provider is not registered: providerId={provider_id}")
            })?;
            if matches.next().is_some() {
                return Err(format!(
                    "requested target provider id is ambiguous: providerId={provider_id}"
                ));
            }
            requested_provider
        }
        super::collect::SourceIndexCollectionScope::CompleteGeneration => {
            return Err(
                "target-provider source envelope publication requires target-provider collection scope"
                    .to_owned(),
            );
        }
    };
    let snapshot = super::api::fresh_target_provider_source_index_snapshot_with_registry(
        publication.project_root,
        &requested_provider.language_id,
        &requested_provider.provider_id,
        &publication.collection_scope,
        publication.provider_registry,
    )?;
    let normalized_extensions = requested_provider
        .source_extensions
        .iter()
        .map(|extension| extension.trim_start_matches('.').to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let provider_owner_count = snapshot
        .source_blobs
        .iter()
        .filter(|(path, _)| {
            normalized_extensions.is_empty()
                || Path::new(path)
                    .extension()
                    .map(|extension| extension.to_string_lossy().to_ascii_lowercase())
                    .is_some_and(|extension| normalized_extensions.contains(&extension))
        })
        .count();
    if provider_owner_count == 0 {
        return Err(format!(
            "target-provider source envelope publication has no provider owners: reason=owners-empty languageId={} providerId={}",
            requested_provider.language_id, requested_provider.provider_id
        ));
    }
    let address_provider_digest = super::provider_envelope::provider_registry_address_digest(
        publication.provider_registry,
        publication.project_root,
    );
    let immutable_envelope = super::publish_provider_source_snapshot_envelope(
        super::ProviderSourceSnapshotEnvelopePublicationV1 {
            snapshot: &snapshot,
            provider_id: requested_provider.provider_id.as_str(),
            address_provider_digest: &address_provider_digest,
            source_extensions: &requested_provider.source_extensions,
            artifact_root: publication.artifact_root,
            provider_workspace_root: publication.project_root,
        },
    )?;
    let canonical_directory = publication
        .artifact_root
        .join("source-snapshot-envelopes")
        .join("v1");
    let canonical_file_name = super::provider_envelope::source_snapshot_envelope_file_name(
        requested_provider.provider_id.as_str(),
        &address_provider_digest,
        &super::provider_envelope::provider_workspace_identity_v1(publication.project_root)?.digest,
    );
    let canonical_envelope = canonical_directory.join(&canonical_file_name);
    let temporary = canonical_directory.join(format!(
        ".{canonical_file_name}.tmp-{}-{}",
        std::process::id(),
        GENERATION_TRANSACTION_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::copy(&immutable_envelope, &temporary).map_err(|error| {
        format!(
            "failed to stage canonical target-provider source envelope {}: {error}",
            temporary.display()
        )
    })?;
    fs::rename(&temporary, &canonical_envelope).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!(
            "failed to publish canonical target-provider source envelope {}: {error}",
            canonical_envelope.display()
        )
    })?;
    Ok(canonical_envelope)
}

/// Typed evidence required to publish one complete source-index generation.
pub struct WorkspaceSearchGenerationPublicationRequestV1<'a> {
    pub collection_scope: super::collect::SourceIndexCollectionScope,
    pub snapshot: &'a CurrentSourceIndexSnapshot,
    pub files: &'a [SourceIndexScopeFile],
    pub registry_evidence: &'a agent_semantic_client_core::ProviderRegistryEvidence,
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
    pub generation_digest: [u8; 32],
    pub fixture_digest: [u8; 32],
    pub workspace_identity_digest: [u8; 32],
}

/// Validate and atomically publish one immutable complete workspace search generation.
pub fn publish_workspace_search_generation_v1(
    publication: WorkspaceSearchGenerationPublicationRequestV1<'_>,
) -> Result<PublishedSourceIndexGenerationV1, String> {
    match &publication.collection_scope {
        super::collect::SourceIndexCollectionScope::CompleteGeneration => {
            publish_complete_workspace_search_generation_v1(publication)
        }
        super::collect::SourceIndexCollectionScope::TargetProvider { .. }
        | super::collect::SourceIndexCollectionScope::TargetProviderId { .. } => Err(
            "workspace search generation publication requires complete-generation collection scope"
                .to_owned(),
        ),
    }
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
    if provider_files.iter().any(|file| {
        file.projection_coverage
            != agent_semantic_client_db::ClientDbSourceIndexProjectionCoverage::Complete
    }) {
        return Err(format!(
            "complete generation owner is missing its provider projection receipt: providerId={}",
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

    let fixture = build_exact_selector_fixture_from_projection_records_v1(
        &publication.generation_identity,
        projection_records.clone(),
    )
    .map_err(|error| format!("failed to build complete exact-selector generation: {error}"))?;
    let fixture_digest = *fixture_digest_v1(&fixture)
        .map_err(|error| format!("failed to identify exact-selector fixture: {error}"))?;
    let generation_digest = publication.generation_identity.generation_digest;
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
    let provider_envelope_path = super::publish_provider_source_snapshot_envelope(
        super::ProviderSourceSnapshotEnvelopePublicationV1 {
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
    let exact_selector_fixture_path = publish_immutable_exact_selector_generation_fixture_v1(
        &staging_directory.join("exact-selector"),
        &fixture,
        publication.workspace_identity,
        generation_digest,
        fixture_digest,
    )
    .map_err(|error| {
        let _ = fs::remove_dir_all(&staging_directory);
        format!("failed to stage exact-selector generation: {error:?}")
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
    if let Err(rename_error) = fs::rename(&staging_directory, &generation_directory) {
        let existing_fixture = generation_directory.join(&fixture_relative_path);
        let existing_envelope = generation_directory.join(&envelope_relative_path);
        let concurrent_generation_is_complete = existing_envelope.is_file()
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
        generation_directory,
        generation_digest,
        fixture_digest,
        workspace_identity_digest: *publication.workspace_identity.identity_digest(),
    })
}

fn digest_matches_bytes(encoded: &str, expected: &[u8; 32]) -> bool {
    encoded == blake3::Hash::from_bytes(*expected).to_hex().as_str()
}

//! Atomically publish one complete, provider-owned source-index generation.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use agent_semantic_client_db::ClientDbSourceIndexScopeFile as SourceIndexScopeFile;
use agent_semantic_content_identity::exact_selector_generation_fixture::{
    ExactSelectorGenerationIdentityV1, ExactSelectorGenerationRecordV1,
    build_exact_selector_generation_fixture_v1, fixture_digest_v1,
};
use agent_semantic_content_identity::workspace_search_identity::WorkspaceSearchIdentityV1;
use agent_semantic_search::exact_selector_generation_fixture::{
    ExactSelectorGenerationMemorySearchV1,
    publish_immutable_exact_selector_generation_fixture_v1,
};

use super::CurrentSourceIndexSnapshot;

static GENERATION_TRANSACTION_ID: AtomicU64 = AtomicU64::new(0);

/// Typed inputs required to publish one complete source-index generation.
pub struct CompleteSourceIndexGenerationPublicationV1<'a> {
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
    pub generation_directory: PathBuf,
    pub provider_envelope_path: PathBuf,
    pub exact_selector_fixture_path: PathBuf,
    pub generation_digest: [u8; 32],
    pub fixture_digest: [u8; 32],
    pub workspace_identity_digest: [u8; 32],
}

pub(super) fn publish_complete_source_index_generation_v1(
    snapshot: &CurrentSourceIndexSnapshot,
    files: &[SourceIndexScopeFile],
    publication: CompleteSourceIndexGenerationPublicationV1<'_>,
) -> Result<PublishedSourceIndexGenerationV1, String> {
    let provider_files = files
        .iter()
        .filter(|file| file.provider_id.as_str() == publication.provider_id)
        .collect::<Vec<_>>();
    if provider_files.is_empty() {
        return Err(format!(
            "complete generation has no provider owners: providerId={}",
            publication.provider_id
        ));
    }
    if provider_files
        .iter()
        .any(|file| file.selector_receipts.is_empty())
    {
        return Err(format!(
            "complete generation owner is missing selector materialization proofs: providerId={}",
            publication.provider_id
        ));
    }
    let proofs_are_from_one_generation = provider_files
        .iter()
        .flat_map(|file| file.selector_receipts.iter())
        .all(|selector| {
            selector.materialization_proof.workspace_root_digest
                == publication.generation_identity.workspace_root_digest
                && selector.materialization_proof.workspace_root_digest
                    == *publication
                        .workspace_identity
                        .source_snapshot_root_digest()
                && selector.materialization_proof.parser_identity_digest
                    == publication.generation_identity.parser_identity_digest
                && selector.materialization_proof.query_pack_digest
                    == publication.generation_identity.query_pack_digest
        });
    if !proofs_are_from_one_generation {
        return Err(format!(
            "selector materialization proofs cross generation identity: providerId={}",
            publication.provider_id
        ));
    }
    let records = provider_files
        .iter()
        .flat_map(|file| file.selector_receipts.iter())
        .map(|selector| {
            ExactSelectorGenerationRecordV1::try_from(&selector.materialization_proof)
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let covered_owner_paths = records
        .iter()
        .map(|record| record.owner_path.as_str())
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
    if covered_owner_paths.len() != provider_owner_paths.len()
        || !provider_owner_paths
            .iter()
            .all(|path| covered_owner_paths.contains(path.as_str()))
    {
        return Err(format!(
            "complete generation owner coverage does not match provider source envelope: providerId={} ownerCount={} coveredOwnerCount={}",
            publication.provider_id,
            provider_owner_paths.len(),
            covered_owner_paths.len()
        ));
    }
    if publication.workspace_identity.owner_count()
        != u32::try_from(provider_owner_paths.len()).unwrap_or(u32::MAX)
        || publication.workspace_identity.selector_count()
            != u32::try_from(records.len()).unwrap_or(u32::MAX)
    {
        return Err(format!(
            "complete generation coverage does not match admitted workspace identity: providerId={}",
            publication.provider_id
        ));
    }

    let fixture = build_exact_selector_generation_fixture_v1(
        &publication.generation_identity,
        records,
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
    let provider_envelope_path = super::api::publish_provider_source_snapshot_envelope(
        snapshot,
        publication.provider_id.to_string(),
        publication.source_extensions,
        &staging_directory,
        publication
            .workspace_identity
            .source_snapshot_root_digest(),
    )
    .inspect_err(|_| {
        let _ = fs::remove_dir_all(&staging_directory);
    })?;
    let exact_selector_fixture_path =
        publish_immutable_exact_selector_generation_fixture_v1(
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
        provider_envelope_path: generation_directory.join(envelope_relative_path),
        exact_selector_fixture_path: generation_directory.join(fixture_relative_path),
        generation_directory,
        generation_digest,
        fixture_digest,
        workspace_identity_digest: *publication.workspace_identity.identity_digest(),
    })
}

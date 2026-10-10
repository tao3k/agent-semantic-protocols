// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! In-memory lookup and immutable publication for exact-selector generation fixtures.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Instant;

use agent_semantic_content_identity::exact_selector_generation_fixture::ExactSelectorGenerationFixtureErrorV1;
use agent_semantic_content_identity::exact_selector_generation_fixture::ExactSelectorGenerationFixtureViewV1;
use agent_semantic_content_identity::exact_selector_generation_fixture::ExactSelectorGenerationRecordViewV1;
use agent_semantic_content_identity::exact_selector_generation_fixture::fixture_digest_v1;
use agent_semantic_content_identity::workspace_search_identity::WorkspaceSearchIdentityV1;

static FIXTURE_TRANSACTION_ID: AtomicU64 = AtomicU64::new(0);

/// Attached in-memory exact-selector fixture for one admitted generation.
#[derive(Clone, Debug)]
pub struct ExactSelectorGenerationMemorySearchV1 {
    bytes: Arc<[u8]>,
    workspace_identity_digest: [u8; 32],
    generation_digest: [u8; 32],
    fixture_digest: [u8; 32],
}

/// Timing and side-effect receipt for one exact-selector lookup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactSelectorGenerationSearchReceiptV1 {
    pub fixture_attach_micros: u128,
    pub selector_resolve_micros: u128,
    pub proof_validate_micros: u128,
    pub render_micros: u128,
    pub total_micros: u128,
    pub subprocesses: u32,
    pub db_opens: u32,
    pub source_files_read: u32,
    pub source_bytes_read: u64,
    pub manifest_writes: u32,
    pub workspace_identity_digest: [u8; 32],
    pub generation_digest: [u8; 32],
    pub fixture_digest: [u8; 32],
    pub hit: bool,
}

/// Typed lookup and immutable-publication failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExactSelectorGenerationSearchErrorV1 {
    Fixture(ExactSelectorGenerationFixtureErrorV1),
    ArtifactIo(String),
    WorkspaceIdentityMismatch(&'static str),
    SelectorNotInActiveGeneration {
        structural_selector: String,
        generation_digest: [u8; 32],
    },
}

impl From<ExactSelectorGenerationFixtureErrorV1> for ExactSelectorGenerationSearchErrorV1 {
    fn from(error: ExactSelectorGenerationFixtureErrorV1) -> Self {
        Self::Fixture(error)
    }
}

impl ExactSelectorGenerationMemorySearchV1 {
    /// Attaches fixture bytes to an exact workspace and generation identity.
    pub fn attach(
        bytes: Arc<[u8]>,
        workspace_identity: &WorkspaceSearchIdentityV1,
        generation_digest: [u8; 32],
        fixture_digest: [u8; 32],
    ) -> Result<Self, ExactSelectorGenerationSearchErrorV1> {
        let view = ExactSelectorGenerationFixtureViewV1::attach(
            bytes.as_ref(),
            &generation_digest,
            &fixture_digest,
        )?;
        if view.language_id() != workspace_identity.language_id() {
            return Err(
                ExactSelectorGenerationSearchErrorV1::WorkspaceIdentityMismatch("languageId"),
            );
        }
        if view.provider_id() != workspace_identity.provider_id() {
            return Err(
                ExactSelectorGenerationSearchErrorV1::WorkspaceIdentityMismatch("providerId"),
            );
        }
        if view.workspace_root_digest() != workspace_identity.source_snapshot_root_digest() {
            return Err(
                ExactSelectorGenerationSearchErrorV1::WorkspaceIdentityMismatch(
                    "sourceSnapshotRootDigest",
                ),
            );
        }
        if view.workspace_identity_digest() != workspace_identity.identity_digest() {
            return Err(
                ExactSelectorGenerationSearchErrorV1::WorkspaceIdentityMismatch(
                    "workspaceIdentityDigest",
                ),
            );
        }
        if view.owner_count() != workspace_identity.owner_count()
            || view.leaf_count() != workspace_identity.leaf_count()
        {
            return Err(
                ExactSelectorGenerationSearchErrorV1::WorkspaceIdentityMismatch("ownerCoverage"),
            );
        }
        if view.selector_count() != workspace_identity.selector_count() {
            return Err(
                ExactSelectorGenerationSearchErrorV1::WorkspaceIdentityMismatch("selectorCoverage"),
            );
        }
        Ok(Self {
            bytes,
            workspace_identity_digest: *workspace_identity.identity_digest(),
            generation_digest,
            fixture_digest,
        })
    }

    /// Loads and attaches an immutable fixture artifact.
    pub fn load_immutable_artifact(
        artifact_path: &Path,
        workspace_identity: &WorkspaceSearchIdentityV1,
        generation_digest: [u8; 32],
        fixture_digest: [u8; 32],
    ) -> Result<Self, ExactSelectorGenerationSearchErrorV1> {
        let bytes = fs::read(artifact_path).map_err(|error| {
            ExactSelectorGenerationSearchErrorV1::ArtifactIo(format!(
                "failed to read exact-selector generation fixture {}: {error}",
                artifact_path.display()
            ))
        })?;
        Self::attach(
            Arc::from(bytes),
            workspace_identity,
            generation_digest,
            fixture_digest,
        )
    }

    /// Resolves one selector without producing a performance receipt.
    pub fn resolve(
        &self,
        structural_selector: &str,
    ) -> Result<ExactSelectorGenerationRecordViewV1<'_>, ExactSelectorGenerationSearchErrorV1> {
        let view = ExactSelectorGenerationFixtureViewV1::attach(
            self.bytes.as_ref(),
            &self.generation_digest,
            &self.fixture_digest,
        )?;
        view.lookup(structural_selector)?.ok_or_else(|| {
            ExactSelectorGenerationSearchErrorV1::SelectorNotInActiveGeneration {
                structural_selector: structural_selector.to_string(),
                generation_digest: self.generation_digest,
            }
        })
    }

    /// Resolves one selector and reports exact lookup costs and side effects.
    #[expect(
        clippy::result_large_err,
        reason = "the V1 typed receipt and error pair is part of the exact-selector contract"
    )]
    pub fn resolve_with_receipt(
        &self,
        structural_selector: &str,
    ) -> Result<
        (
            ExactSelectorGenerationRecordViewV1<'_>,
            ExactSelectorGenerationSearchReceiptV1,
        ),
        (
            ExactSelectorGenerationSearchErrorV1,
            ExactSelectorGenerationSearchReceiptV1,
        ),
    > {
        self.resolve_with_receipt_inner(structural_selector)
    }

    #[expect(
        clippy::result_large_err,
        reason = "the V1 typed receipt and error pair is part of the exact-selector contract"
    )]
    fn resolve_with_receipt_inner(
        &self,
        structural_selector: &str,
    ) -> Result<
        (
            ExactSelectorGenerationRecordViewV1<'_>,
            ExactSelectorGenerationSearchReceiptV1,
        ),
        (
            ExactSelectorGenerationSearchErrorV1,
            ExactSelectorGenerationSearchReceiptV1,
        ),
    > {
        let total_started = Instant::now();
        let attach_started = Instant::now();
        let view = match ExactSelectorGenerationFixtureViewV1::attach(
            self.bytes.as_ref(),
            &self.generation_digest,
            &self.fixture_digest,
        ) {
            Ok(view) => view,
            Err(error) => {
                let receipt = self.receipt(
                    attach_started.elapsed().as_micros(),
                    0,
                    0,
                    0,
                    total_started.elapsed().as_micros(),
                    false,
                );
                return Err((error.into(), receipt));
            }
        };
        let fixture_attach_micros = attach_started.elapsed().as_micros();
        let resolve_started = Instant::now();
        let record = match view.lookup(structural_selector) {
            Ok(Some(record)) => record,
            Ok(None) => {
                let selector_resolve_micros = resolve_started.elapsed().as_micros();
                let receipt = self.receipt(
                    fixture_attach_micros,
                    selector_resolve_micros,
                    0,
                    0,
                    total_started.elapsed().as_micros(),
                    false,
                );
                return Err((
                    ExactSelectorGenerationSearchErrorV1::SelectorNotInActiveGeneration {
                        structural_selector: structural_selector.to_string(),
                        generation_digest: self.generation_digest,
                    },
                    receipt,
                ));
            }
            Err(error) => {
                let selector_resolve_micros = resolve_started.elapsed().as_micros();
                let receipt = self.receipt(
                    fixture_attach_micros,
                    selector_resolve_micros,
                    0,
                    0,
                    total_started.elapsed().as_micros(),
                    false,
                );
                return Err((error.into(), receipt));
            }
        };
        let selector_resolve_micros = resolve_started.elapsed().as_micros();
        let proof_started = Instant::now();
        std::hint::black_box(record.owner_subtree_digest);
        std::hint::black_box(record.source_blob_digest);
        std::hint::black_box(record.normalized_parser_facts_digest);
        let proof_validate_micros = proof_started.elapsed().as_micros();
        let render_started = Instant::now();
        std::hint::black_box(record.projection);
        let render_micros = render_started.elapsed().as_micros();
        let receipt = self.receipt(
            fixture_attach_micros,
            selector_resolve_micros,
            proof_validate_micros,
            render_micros,
            total_started.elapsed().as_micros(),
            true,
        );
        Ok((record, receipt))
    }

    fn receipt(
        &self,
        fixture_attach_micros: u128,
        selector_resolve_micros: u128,
        proof_validate_micros: u128,
        render_micros: u128,
        total_micros: u128,
        hit: bool,
    ) -> ExactSelectorGenerationSearchReceiptV1 {
        ExactSelectorGenerationSearchReceiptV1 {
            fixture_attach_micros,
            selector_resolve_micros,
            proof_validate_micros,
            render_micros,
            total_micros,
            subprocesses: 0,
            db_opens: 0,
            source_files_read: 0,
            source_bytes_read: 0,
            manifest_writes: 0,
            workspace_identity_digest: self.workspace_identity_digest,
            generation_digest: self.generation_digest,
            fixture_digest: self.fixture_digest,
            hit,
        }
    }

    /// Returns the exact workspace identity digest.
    pub fn workspace_identity_digest(&self) -> &[u8; 32] {
        &self.workspace_identity_digest
    }
}

/// Named immutable-publication input for one exact-selector fixture.
pub struct ExactSelectorFixturePublicationV1<'a> {
    /// Content-addressed generation directory.
    pub generation_directory: &'a Path,
    /// Canonical fixture bytes.
    pub fixture: &'a [u8],
    /// Exact workspace identity.
    pub workspace_identity: &'a WorkspaceSearchIdentityV1,
    /// Exact generation digest.
    pub generation_digest: [u8; 32],
    /// Canonical fixture digest.
    pub fixture_digest: [u8; 32],
}

/// Atomically publishes a validated immutable exact-selector fixture.
pub fn publish_immutable_exact_selector_generation_fixture_v1(
    publication: ExactSelectorFixturePublicationV1<'_>,
) -> Result<PathBuf, ExactSelectorGenerationSearchErrorV1> {
    publish_exact_selector_fixture_transaction(publication)
}

fn publish_exact_selector_fixture_transaction(
    publication: ExactSelectorFixturePublicationV1<'_>,
) -> Result<PathBuf, ExactSelectorGenerationSearchErrorV1> {
    let ExactSelectorFixturePublicationV1 {
        generation_directory,
        fixture,
        workspace_identity,
        generation_digest,
        fixture_digest,
    } = publication;
    ExactSelectorGenerationMemorySearchV1::attach(
        Arc::from(fixture),
        workspace_identity,
        generation_digest,
        fixture_digest,
    )?;
    fs::create_dir_all(generation_directory).map_err(|error| {
        ExactSelectorGenerationSearchErrorV1::ArtifactIo(format!(
            "failed to create exact-selector generation directory {}: {error}",
            generation_directory.display()
        ))
    })?;
    let actual_fixture_digest = fixture_digest_v1(fixture)?;
    if *actual_fixture_digest != fixture_digest {
        return Err(ExactSelectorGenerationSearchErrorV1::ArtifactIo(
            "exact-selector fixture digest does not match admitted generation".to_string(),
        ));
    }
    let digest_hex = actual_fixture_digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let artifact_path = generation_directory.join("exact-selector-generation.v1.bin");
    if artifact_path.exists() {
        let existing = fs::read(&artifact_path).map_err(|error| {
            ExactSelectorGenerationSearchErrorV1::ArtifactIo(format!(
                "failed to validate existing exact-selector fixture {}: {error}",
                artifact_path.display()
            ))
        })?;
        if existing != fixture {
            return Err(ExactSelectorGenerationSearchErrorV1::ArtifactIo(format!(
                "content-addressed exact-selector fixture collision: {}",
                artifact_path.display()
            )));
        }
        return Ok(artifact_path);
    }
    let temporary_path = generation_directory.join(format!(
        ".exact-selector-generation-{digest_hex}.tmp-{}-{}",
        std::process::id(),
        FIXTURE_TRANSACTION_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let mut temporary = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary_path)
        .map_err(|error| {
            ExactSelectorGenerationSearchErrorV1::ArtifactIo(format!(
                "failed to create exact-selector fixture transaction {}: {error}",
                temporary_path.display()
            ))
        })?;
    temporary.write_all(fixture).map_err(|error| {
        ExactSelectorGenerationSearchErrorV1::ArtifactIo(format!(
            "failed to write exact-selector fixture transaction {}: {error}",
            temporary_path.display()
        ))
    })?;
    temporary.sync_all().map_err(|error| {
        ExactSelectorGenerationSearchErrorV1::ArtifactIo(format!(
            "failed to sync exact-selector fixture transaction {}: {error}",
            temporary_path.display()
        ))
    })?;
    if let Err(rename_error) = fs::rename(&temporary_path, &artifact_path) {
        let concurrent = fs::read(&artifact_path).ok();
        if concurrent.as_deref() != Some(fixture) {
            return Err(ExactSelectorGenerationSearchErrorV1::ArtifactIo(format!(
                "failed to commit exact-selector fixture {} -> {}: {rename_error}",
                temporary_path.display(),
                artifact_path.display()
            )));
        }
        fs::remove_file(&temporary_path).map_err(|error| {
            ExactSelectorGenerationSearchErrorV1::ArtifactIo(format!(
                "failed to remove superseded exact-selector fixture transaction {}: {error}",
                temporary_path.display()
            ))
        })?;
    }
    fs::File::open(generation_directory)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            ExactSelectorGenerationSearchErrorV1::ArtifactIo(format!(
                "failed to sync exact-selector generation directory {}: {error}",
                generation_directory.display()
            ))
        })?;
    Ok(artifact_path)
}

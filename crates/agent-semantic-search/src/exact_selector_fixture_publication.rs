use std::fs::File;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use agent_semantic_content_identity::exact_selector_cache::ExactSelectorProjectionRecordV1;
use agent_semantic_content_identity::exact_selector_generation_fixture::ExactSelectorGenerationIdentityV1;
use agent_semantic_content_identity::exact_selector_generation_fixture::ExactSelectorGenerationRecordV1;
use agent_semantic_content_identity::exact_selector_generation_fixture::ExactSelectorProjectionModeV1;
use agent_semantic_content_identity::exact_selector_generation_fixture::build_exact_selector_generation_fixture_v1;
use agent_semantic_content_identity::exact_selector_merkle::ExactProjectionModeV1;

use crate::active_exact_selector_fixture::ExactSelectorFixtureArtifactInput;

pub const EXACT_SELECTOR_FIXTURE_ARTIFACT_KIND: &str = "exact-selector-generation-fixture";

static PUBLICATION_NONCE: AtomicU64 = AtomicU64::new(0);
static PUBLICATION_SHARDS: LazyLock<[Mutex<std::collections::HashSet<String>>; 64]> =
    LazyLock::new(|| std::array::from_fn(|_| Mutex::new(std::collections::HashSet::new())));

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactSelectorFixturePublicationReceiptV1 {
    pub logical_path: String,
    pub materialized_path: PathBuf,
    pub artifact_kind: agent_semantic_content_identity::active_artifact_merkle::ActiveArtifactKind,
    pub artifact_digest: String,
    pub generation_digest: [u8; 32],
}

impl From<&ExactSelectorFixturePublicationReceiptV1> for ExactSelectorFixtureArtifactInput {
    fn from(receipt: &ExactSelectorFixturePublicationReceiptV1) -> Self {
        Self {
            logical_path: receipt.logical_path.clone(),
            materialized_path: receipt.materialized_path.clone(),
            artifact_kind: receipt.artifact_kind,
            artifact_digest: receipt.artifact_digest.clone(),
        }
    }
}

pub fn build_exact_selector_fixture_from_projection_records_v1(
    identity: &ExactSelectorGenerationIdentityV1,
    records: Vec<ExactSelectorProjectionRecordV1>,
) -> Result<Vec<u8>, String> {
    let records = records
        .into_iter()
        .map(|record| {
            let proof = &record.proof;
            Ok(ExactSelectorGenerationRecordV1 {
                structural_selector: proof.structural_selector().to_owned(),
                owner_path: proof.owner_path().to_owned(),
                source_blob_digest: parse_digest(proof.source_blob_digest().as_str())?,
                owner_subtree_digest: parse_digest(proof.owner_subtree_digest().as_str())?,
                normalized_parser_facts_digest: parse_digest(proof.parser_fact_digest().as_str())?,
                projection_mode: match proof.projection_mode() {
                    ExactProjectionModeV1::Code | ExactProjectionModeV1::Verbatim => {
                        ExactSelectorProjectionModeV1::Source
                    }
                    ExactProjectionModeV1::Skeleton => {
                        ExactSelectorProjectionModeV1::CallableSkeleton
                    }
                    ExactProjectionModeV1::Names => {
                        return Err(
                            "names-only projection cannot publish a typed exact-selector generation"
                                .to_string(),
                        );
                    }
                },
                source_byte_range: record.source_byte_range,
                projection: record.projection_payload,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    build_exact_selector_generation_fixture_v1(identity, records).map_err(|error| error.to_string())
}

pub fn publish_exact_selector_fixture_v1(
    artifact_root: &Path,
    identity: &ExactSelectorGenerationIdentityV1,
    records: Vec<ExactSelectorProjectionRecordV1>,
) -> Result<ExactSelectorFixturePublicationReceiptV1, String> {
    let fixture = build_exact_selector_fixture_from_projection_records_v1(identity, records)?;
    publish_exact_selector_fixture_bytes_v1(artifact_root, identity.generation_digest, fixture)
}

pub fn publish_exact_selector_generation_records_v1(
    artifact_root: &Path,
    identity: &ExactSelectorGenerationIdentityV1,
    records: Vec<ExactSelectorGenerationRecordV1>,
) -> Result<ExactSelectorFixturePublicationReceiptV1, String> {
    let fixture = build_exact_selector_generation_fixture_v1(identity, records)
        .map_err(|error| error.to_string())?;
    publish_exact_selector_fixture_bytes_v1(artifact_root, identity.generation_digest, fixture)
}

fn publish_exact_selector_fixture_bytes_v1(
    artifact_root: &Path,
    generation_digest: [u8; 32],
    fixture: Vec<u8>,
) -> Result<ExactSelectorFixturePublicationReceiptV1, String> {
    let artifact_digest = blake3::hash(&fixture).to_hex().to_string();
    let generation_digest_hex = blake3::Hash::from_bytes(generation_digest)
        .to_hex()
        .to_string();
    let generation_root = artifact_root
        .join("exact-selector-generation")
        .join(&generation_digest_hex);
    std::fs::create_dir_all(&generation_root).map_err(|error| {
        format!(
            "failed to create exact selector generation root {}: {error}",
            generation_root.display()
        )
    })?;
    let materialized_path = generation_root.join(format!("{artifact_digest}.fixture"));
    let shard = usize::from(blake3::hash(&fixture).as_bytes()[0]) % PUBLICATION_SHARDS.len();
    let mut published_digests = PUBLICATION_SHARDS[shard]
        .lock()
        .map_err(|_| format!("exact selector publication shard {shard} is poisoned"))?;

    if !published_digests.contains(&artifact_digest) {
        if materialized_path.exists() {
            verify_artifact_digest(&materialized_path, &artifact_digest)?;
        } else {
            atomic_publish(&generation_root, &materialized_path, &fixture)?;
            verify_artifact_digest(&materialized_path, &artifact_digest)?;
        }
        published_digests.insert(artifact_digest.clone());
    }

    Ok(ExactSelectorFixturePublicationReceiptV1 {
        logical_path: format!(
            "exact-selector-generation/{generation_digest_hex}/{artifact_digest}.fixture"
        ),
        materialized_path,
        artifact_kind: agent_semantic_content_identity::active_artifact_merkle::ActiveArtifactKind::ExactSelectorGenerationFixture,
        artifact_digest,
        generation_digest,
    })
}

fn atomic_publish(
    generation_root: &Path,
    materialized_path: &Path,
    fixture: &[u8],
) -> Result<(), String> {
    let nonce = PUBLICATION_NONCE.fetch_add(1, Ordering::Relaxed);
    let temporary_path =
        generation_root.join(format!(".publication-{}-{nonce}.tmp", std::process::id()));
    let result = (|| {
        let mut temporary = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary_path)
            .map_err(|error| {
                format!(
                    "failed to create exact selector temporary artifact {}: {error}",
                    temporary_path.display()
                )
            })?;
        temporary.write_all(fixture).map_err(|error| {
            format!(
                "failed to write exact selector temporary artifact {}: {error}",
                temporary_path.display()
            )
        })?;
        temporary.sync_all().map_err(|error| {
            format!(
                "failed to sync exact selector temporary artifact {}: {error}",
                temporary_path.display()
            )
        })?;
        std::fs::rename(&temporary_path, materialized_path).map_err(|error| {
            format!(
                "failed to commit exact selector artifact {}: {error}",
                materialized_path.display()
            )
        })?;
        File::open(generation_root)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| {
                format!(
                    "failed to sync exact selector generation root {}: {error}",
                    generation_root.display()
                )
            })
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary_path);
    }
    result
}

fn verify_artifact_digest(path: &Path, expected_digest: &str) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|error| {
        format!(
            "failed to read exact selector artifact {}: {error}",
            path.display()
        )
    })?;
    let actual_digest = blake3::hash(&bytes).to_hex().to_string();
    if actual_digest != expected_digest {
        return Err(format!(
            "exact selector artifact digest mismatch: path={} expected={} actual={}",
            path.display(),
            expected_digest,
            actual_digest
        ));
    }
    Ok(())
}

fn parse_digest(value: &str) -> Result<[u8; 32], String> {
    blake3::Hash::from_hex(value)
        .map(|digest| *digest.as_bytes())
        .map_err(|error| format!("invalid exact selector digest {value}: {error}"))
}

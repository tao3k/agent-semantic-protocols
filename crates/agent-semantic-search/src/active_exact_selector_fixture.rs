use agent_semantic_content_identity::active_artifact_merkle_v1::ActiveArtifactKindV1;
use agent_semantic_content_identity::active_artifact_merkle_v1::ActiveAspArtifactReceiptV1;
use agent_semantic_hook::ActiveAspArtifactInput;

use crate::exact_selector_fixture_memory::ExactSelectorFixtureFileBackendV1;

pub fn exact_selector_fixture_active_artifact_input_v1(
    receipt: &ActiveAspArtifactReceiptV1,
) -> Result<ActiveAspArtifactInput, String> {
    let leaf = receipt
        .leaves()
        .iter()
        .find(|leaf| leaf.artifact_kind() == ActiveArtifactKindV1::ExactSelectorGenerationFixture)
        .ok_or_else(|| {
            "exact selector generation state=cold-required reasonKind=active-fixture-missing"
                .to_owned()
        })?;
    Ok(ActiveAspArtifactInput {
        logical_path: leaf.logical_path().to_owned(),
        materialized_path: std::path::PathBuf::from(leaf.materialized_path()),
        artifact_kind: leaf.artifact_kind(),
        artifact_digest: leaf.artifact_digest().as_str().to_owned(),
    })
}

pub fn exact_selector_fixture_backend_from_active_artifact_v1(
    artifact: &ActiveAspArtifactInput,
) -> Result<ExactSelectorFixtureFileBackendV1, String> {
    if artifact.artifact_kind != ActiveArtifactKindV1::ExactSelectorGenerationFixture {
        return Err(format!(
            "active artifact is not an exact selector generation fixture: kind={}",
            artifact.artifact_kind.canonical_name()
        ));
    }
    blake3::Hash::from_hex(&artifact.artifact_digest).map_err(|error| {
        format!(
            "invalid exact selector active artifact digest {}: {error}",
            artifact.artifact_digest
        )
    })?;
    let generation_digest = exact_selector_generation_digest_from_logical_path_v1(
        &artifact.logical_path,
        &artifact.artifact_digest,
    )?;
    Ok(ExactSelectorFixtureFileBackendV1::new(
        artifact.materialized_path.clone(),
        artifact.artifact_digest.clone(),
        generation_digest,
    ))
}

fn exact_selector_generation_digest_from_logical_path_v1(
    logical_path: &str,
    artifact_digest: &str,
) -> Result<[u8; 32], String> {
    let components = std::path::Path::new(logical_path)
        .components()
        .map(|component| component.as_os_str().to_str())
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| {
            format!("exact selector active artifact logical path is not UTF-8: {logical_path}")
        })?;
    let [root, generation_digest, artifact_file] = components.as_slice() else {
        return Err(format!(
            "invalid exact selector active artifact logical path: {logical_path}"
        ));
    };
    if *root != "exact-selector-generation"
        || *artifact_file != format!("{artifact_digest}.fixture")
    {
        return Err(format!(
            "non-canonical exact selector active artifact logical path: {logical_path}"
        ));
    }
    blake3::Hash::from_hex(generation_digest)
        .map(|digest| *digest.as_bytes())
        .map_err(|error| {
            format!(
                "invalid exact selector generation digest in logical path {logical_path}: {error}"
            )
        })
}

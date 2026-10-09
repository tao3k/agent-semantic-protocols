// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later
//! Admit the running Runtime artifact and its explicitly bundled search owner.

pub(super) async fn admit_execution_owner(
    state_home: &std::path::Path,
    runtime_artifact_path: &std::path::Path,
) -> Result<agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity, String> {
    let runtime_binary_identity = if let Some(expected_digest) =
        std::env::var_os("ASP_RUNTIME_BINARY_CONTENT_DIGEST")
    {
        let expected_digest = expected_digest.to_string_lossy().into_owned();
        let expected_digest =
            agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::parse(
                &expected_digest,
            )
            .map_err(|error| {
                format!(
                    "owner=runtime_server_daemon field=expectedDigest reasonKind=runtime-binary-identity-invalid {error}"
                )
            })?;
        let canonical_artifact = tokio::fs::canonicalize(runtime_artifact_path)
            .await
            .map_err(|error| format!("canonicalize candidate Runtime artifact: {error}"))?;
        let observed_digest =
            agent_semantic_content_identity::file_content_digest_v1(&canonical_artifact).map_err(
                |error| {
                    format!(
                        "owner=runtime_server_daemon field=currentExecutable reasonKind=runtime-binary-content-read-failed {error}"
                    )
                },
            )?;
        if observed_digest != expected_digest.content_digest().as_str() {
            return Err(serde_json::json!({
                "schemaId": "agent.semantic-protocols.runtime-server-generation-mismatch",
                "schemaVersion": "1",
                "reasonKind": "runtime-server-generation-mismatch",
                "expectedBinaryContentDigest": expected_digest.to_string(),
                "observedBinaryContentDigest": observed_digest,
            })
            .to_string());
        }
        agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity::Content {
            digest: expected_digest,
        }
    } else {
        let active_bundle =
            agent_semantic_artifacts::runtime_artifact_slots::verify_runtime_artifact_bound_bundle(
                &agent_semantic_artifacts::RuntimeArtifactStateLayout::new(state_home)
                    .active_slot(),
            )
            .await?;
        let active_asp = active_bundle
            .member_path("asp")
            .ok_or_else(|| "active Runtime bundle omits asp".to_owned())?;
        let active_identity =
            agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity::from_bytes(
                &tokio::fs::read(&active_asp).await.map_err(|error| {
                    format!("read active Runtime asp {}: {error}", active_asp.display())
                })?,
            );
        let invoker_identity =
            agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity::from_bytes(
                &tokio::fs::read(runtime_artifact_path)
                    .await
                    .map_err(|error| {
                        format!(
                            "read Runtime Server executable {}: {error}",
                            runtime_artifact_path.display()
                        )
                    })?,
            );
        if invoker_identity != active_identity {
            return Err(
                "Runtime Server executable differs from the active bound bundle".to_owned(),
            );
        }
        active_identity
    };
    #[cfg(feature = "mrr-data-search-composition")]
    {
        let canonical_artifact = tokio::fs::canonicalize(runtime_artifact_path)
            .await
            .map_err(|error| format!("resolve Runtime bundle executable: {error}"))?;
        let root = canonical_artifact
            .parent()
            .ok_or_else(|| "Runtime executable omits bundle directory".to_owned())?;
        let bundle =
            agent_semantic_artifacts::runtime_artifact_slots::verify_runtime_artifact_bound_bundle(
                root,
            )
            .await?;
        if bundle.member_path("asp").as_deref() != Some(canonical_artifact.as_path()) {
            return Err("reasonKind=runtime-bundle-executable-mismatch Running ASP is not the admitted bundle member".to_owned());
        }
        let owner = bundle.member_path("mrr-search").ok_or_else(|| {
            "reasonKind=mrr-search-owner-missing Runtime bundle omits MRR search owner".to_owned()
        })?;
        agent_semantic_runtime_server::configure_data_search_execution(&owner)?;
    }

    Ok(runtime_binary_identity)
}

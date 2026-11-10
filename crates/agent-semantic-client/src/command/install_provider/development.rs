// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Development-checkout provider build delegation.

use std::path::Path;

pub(super) struct DevelopmentArtifactProvenance {
    pub source_snapshot_root: String,
    pub source_leaf_count: usize,
    pub provider_digest: String,
    pub build_recipe_digest: String,
}

pub(super) fn capture_development_artifact_provenance(
    dev_root: &Path,
    registration: &crate::command::provider_install_registry::ProviderInstallRegistration,
    target: &str,
) -> Result<DevelopmentArtifactProvenance, String> {
    let provider_source_root = dev_root
        .join(&registration.source_root)
        .canonicalize()
        .map_err(|error| {
            format!(
                "canonicalize provider development sourceRoot {}: {error}",
                registration.source_root
            )
        })?;
    if !provider_source_root.starts_with(dev_root) {
        return Err(format!(
            "provider development sourceRoot escaped [dev].root: language={} sourceRoot={}",
            registration.language_id.as_str(),
            provider_source_root.display()
        ));
    }
    let snapshot =
        agent_semantic_runtime::git::discover_repository_candidate_snapshot(&provider_source_root)
            .map_err(|error| format!("capture provider source candidate scope: {error}"))?
            .ok_or_else(|| {
                format!(
                    "provider development sourceRoot is not a Git worktree: {}",
                    provider_source_root.display()
                )
            })?;
    let file_hashes = snapshot
        .candidates
        .iter()
        .filter_map(|candidate| {
            let path = provider_source_root.join(&candidate.path);
            path.is_file().then_some((candidate.path.as_path(), path))
        })
        .map(|(relative_path, path)| {
            agent_semantic_content_identity::file_content_digest_v1(&path)
                .map(|digest| (relative_path.to_string_lossy().into_owned(), digest))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let source_leaf_count = file_hashes.len();
    let source_snapshot =
        agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(file_hashes);
    let provider_digest =
        crate::command::provider_install_registry::provider_install_registration_digest(
            registration,
        )?;
    let build_descriptor = registration.workspace_install.as_str();
    let build_descriptor = provider_source_root.join(build_descriptor);
    let build_descriptor_digest =
        agent_semantic_content_identity::file_content_digest_v1(&build_descriptor)?;
    let build_recipe_digest = format!(
        "blake3-256:{}",
        blake3::hash(
            format!(
                "{}\0{}\0{}\0{}\0{}",
                registration.build_binding,
                registration.language_id.as_str(),
                registration.source_root,
                target,
                build_descriptor_digest
            )
            .as_bytes()
        )
        .to_hex()
    );
    Ok(DevelopmentArtifactProvenance {
        source_snapshot_root: source_snapshot.root_digest().to_string(),
        source_leaf_count,
        provider_digest,
        build_recipe_digest,
    })
}

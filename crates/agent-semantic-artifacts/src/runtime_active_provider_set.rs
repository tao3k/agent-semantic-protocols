// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Typed provider projection from the single verified active Runtime bundle.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveRuntimeProviderArtifact {
    pub language_id: String,
    pub provider_id: String,
    pub materialized_path: PathBuf,
    pub artifact_digest: String,
    pub artifact_metadata_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveRuntimeProviderSet {
    pub runtime_bundle_digest: String,
    pub providers: Vec<ActiveRuntimeProviderArtifact>,
}

pub fn load_active_runtime_provider_set(
    state_home: &Path,
) -> Result<ActiveRuntimeProviderSet, String> {
    let layout = crate::RuntimeArtifactStateLayout::new(state_home);
    let active = std::fs::canonicalize(layout.active_slot()).map_err(|error| {
        format!(
            "state=runtime-provider-artifacts-unavailable reasonKind=runtime-active-generation-unavailable path={} error={error}",
            layout.active_slot().display()
        )
    })?;
    let bundle = crate::runtime_artifact_slots::verify_runtime_artifact_bundle_blocking(&active)?;
    provider_set_from_verified_bundle(bundle)
}

pub async fn load_active_runtime_provider_set_async(
    state_home: &Path,
) -> Result<ActiveRuntimeProviderSet, String> {
    let state_home = state_home.to_path_buf();
    tokio::task::spawn_blocking(move || load_active_runtime_provider_set(&state_home))
        .await
        .map_err(|error| format!("load active Runtime provider set task failed: {error}"))?
}

fn provider_set_from_verified_bundle(
    bundle: crate::runtime_artifact_slots::VerifiedRuntimeArtifactBundle,
) -> Result<ActiveRuntimeProviderSet, String> {
    let mut providers = Vec::new();
    for registration in agent_semantic_provider_protocol::builtin_provider_registrations()? {
        let member_name = registration.provider_id.as_str();
        let Some(member_digest) = bundle.member_digest(member_name) else {
            continue;
        };
        let materialized_path = bundle
            .member_path(member_name)
            .ok_or_else(|| format!("verified Runtime member vanished: {member_name}"))?;
        providers.push(ActiveRuntimeProviderArtifact {
            language_id: registration.language_id,
            provider_id: registration.provider_id,
            artifact_metadata_digest:
                agent_semantic_content_identity::file_artifact_metadata_digest_v1(
                    &materialized_path,
                )?,
            materialized_path,
            artifact_digest: member_digest.to_string(),
        });
    }
    providers.sort_by(|left, right| left.language_id.cmp(&right.language_id));
    Ok(ActiveRuntimeProviderSet {
        runtime_bundle_digest: bundle.bundle_digest().to_string(),
        providers,
    })
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn provider_set_is_derived_from_active_bundle_without_a_side_document() {
        let state_home = tempfile::tempdir().expect("state home");
        let sources = state_home.path().join("sources");
        std::fs::create_dir_all(&sources).expect("sources");
        let asp = sources.join("asp");
        let hook = sources.join("asp-hook");
        let rust = sources.join("asp-rust");
        std::fs::write(&asp, b"asp").expect("asp");
        std::fs::write(&hook, b"hook").expect("hook");
        std::fs::write(&rust, b"rust").expect("rust");
        let members = [
            crate::runtime_artifact_publication::RuntimeArtifactBundleMemberSource {
                name: "asp-hook",
                source: &hook,
            },
            crate::runtime_artifact_publication::RuntimeArtifactBundleMemberSource {
                name: "asp-rust",
                source: &rust,
            },
        ];
        let published =
            crate::runtime_artifact_publication::publish_runtime_artifact_bundle_members(
                state_home.path(),
                &asp,
                &state_home.path().join("runtime/bin/asp"),
                "dev",
                &members,
            )
            .await
            .expect("publish bundle");

        let providers = super::load_active_runtime_provider_set(state_home.path())
            .expect("active provider set");
        assert_eq!(
            providers.runtime_bundle_digest,
            published.bundle_digest.to_string()
        );
        assert_eq!(providers.providers.len(), 1);
        assert_eq!(providers.providers[0].provider_id, "asp-rust");
        assert!(
            providers.providers[0].materialized_path.starts_with(
                crate::RuntimeArtifactStateLayout::new(state_home.path())
                    .generation_store()
                    .canonicalize()
                    .expect("canonical generation store")
            )
        );
        assert!(
            !state_home
                .path()
                .join("runtime/installed-provider-artifacts.json")
                .exists()
        );
        assert!(
            !state_home
                .path()
                .join("runtime/installed-provider-binding.v1.json")
                .exists()
        );
    }
}

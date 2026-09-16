// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Atomic Provider replacement inside the complete bound Runtime bundle.

use std::path::Path;

use super::PredecessorReceiptAdmission;
use super::RuntimeArtifactBundleMemberSource;
use super::RuntimeArtifactPublicationReceipt;
use super::publish_runtime_artifact_with_before_guard;

/// Replace one Provider executable and regenerate its registration and
/// artifact closure in the same candidate transaction.
pub async fn publish_runtime_artifact_bound_provider_member_from_active(
    state_home: &Path,
    member_name: &str,
    source: &Path,
    artifact_mode: &str,
) -> Result<RuntimeArtifactPublicationReceipt, String> {
    let member_path = Path::new(member_name);
    if member_path.components().count() != 1
        || member_name == "."
        || member_name == ".."
        || member_name == "asp"
    {
        return Err(format!(
            "invalid Runtime replacement bundle member `{member_name}`"
        ));
    }
    let layout = crate::RuntimeArtifactStateLayout::new(state_home);
    let active_bundle = std::fs::canonicalize(layout.active_slot()).map_err(|error| {
        format!(
            "reasonKind=runtime-active-generation-unavailable path={} error={error}",
            layout.active_slot().display()
        )
    })?;
    let verified = crate::runtime_artifact_slots::verify_runtime_artifact_bound_bundle_for_provider_replacement(
        &active_bundle,
        member_name,
    )
    .await?;
    let predecessor_closure = crate::runtime_artifact_execution_closure::RuntimeArtifactExecutionClosure::from_materialized_members(
        &active_bundle,
        verified.members(),
    )?;
    let closure_names = [
        "provider-registration.json",
        "provider-artifact-set",
        "evaluator-policy.json",
        "evaluator-abi.json",
        "schema-bundle.json",
    ];
    let mut successor_members = verified.members().clone();
    successor_members.insert(
        member_name.to_owned(),
        crate::runtime_artifact_slots::runtime_artifact_candidate_digest(source).await?,
    );
    for name in closure_names {
        successor_members.remove(name);
    }
    let successor_closure = crate::runtime_artifact_execution_closure::RuntimeArtifactExecutionClosure::from_runtime_bundle_members(
        &successor_members,
        predecessor_closure.evaluator_policy.entries,
        predecessor_closure.evaluator_abi.entries,
        predecessor_closure.schema_bundle.entries,
    )?;
    let execution_binding = successor_closure.binding()?;
    let closure_staging = layout.provider_staging().join(format!(
        ".runtime-execution-closure-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| format!("system time before Unix epoch: {error}"))?
            .as_nanos()
    ));
    std::fs::create_dir_all(&closure_staging).map_err(|error| {
        format!(
            "create Runtime execution closure staging {}: {error}",
            closure_staging.display()
        )
    })?;
    let result = async {
        let closure_members = successor_closure.materialized_members()?;
        for (name, bytes) in &closure_members {
            std::fs::write(closure_staging.join(name), bytes).map_err(|error| {
                format!("write Runtime execution closure successor `{name}`: {error}")
            })?;
        }
        let asp_source = active_bundle.join("asp");
        let mut owned_members = successor_members
            .keys()
            .filter(|name| name.as_str() != "asp" && name.as_str() != member_name)
            .map(|name| (name.clone(), active_bundle.join(name)))
            .collect::<Vec<_>>();
        owned_members.push((member_name.to_owned(), source.to_path_buf()));
        owned_members.extend(
            closure_members
                .keys()
                .map(|name| ((*name).to_owned(), closure_staging.join(name))),
        );
        owned_members.sort_by(|left, right| left.0.cmp(&right.0));
        let member_sources = owned_members
            .iter()
            .map(|(name, source)| RuntimeArtifactBundleMemberSource {
                name: name.as_str(),
                source,
            })
            .collect::<Vec<_>>();
        publish_runtime_artifact_with_before_guard(
            state_home,
            &asp_source,
            &state_home.join("runtime/bin/asp"),
            artifact_mode,
            &member_sources,
            Some(&execution_binding),
            Some(&active_bundle),
            PredecessorReceiptAdmission::PreverifiedProviderReplacement(verified.bundle_digest()),
            &[member_name],
            || async {},
        )
        .await
    }
    .await;
    let _ = std::fs::remove_dir_all(&closure_staging);
    result
}

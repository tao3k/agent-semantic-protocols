use std::path::Path;

use serde::{Deserialize, Serialize};

pub const RUNTIME_STATE_CLEANUP_PLAN_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-state-cleanup-plan";
pub const RUNTIME_STATE_CLEANUP_PLAN_SCHEMA_VERSION: &str = "1";
pub const RUNTIME_STATE_CLEANUP_COMMIT_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-state-cleanup-commit-receipt";

const LEGACY_RUNTIME_AUTHORITIES: &[&str] = &[
    "activation",
    "artifact-identities",
    "bin",
    "installed-provider-artifacts.json",
    "installed-provider-binding.v1.json",
    "leases",
    "locks",
    "profiles",
    "provider-catalog.v1.json",
    "provider-artifacts",
    "provider-locks",
    "providers",
    "resident",
    "state",
];

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStateCleanupPlanV1 {
    pub schema_id: String,
    pub schema_version: String,
    pub state: RuntimeStateCleanupPlanState,
    pub plan_digest: String,
    pub expected_active_bundle_digest: Option<String>,
    pub expected_healthy_bundle_digest: Option<String>,
    pub entries: Vec<RuntimeStateCleanupPlanEntryV1>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeStateCleanupPlanState {
    Planned,
    Committed,
    Rejected,
    Cancelled,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStateCleanupPlanEntryV1 {
    pub relative_path: String,
    pub reason_kind: RuntimeStateCleanupReasonKind,
    pub evidence_class: RuntimeStateCleanupEvidenceClass,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStateCleanupCommitReceiptV1 {
    pub schema_id: String,
    pub schema_version: String,
    pub state: RuntimeStateCleanupPlanState,
    pub plan_digest: String,
    pub reason_kind: String,
    pub deleted_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeStateCleanupReasonKind {
    LegacyRuntimeAuthority,
    UnreachableContent,
    ExpiredDiagnostic,
    AbandonedStage,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeStateCleanupEvidenceClass {
    ActiveHealthyUnreachable,
    LeaseUnreachable,
    HandoffUnreachable,
    RetentionExpired,
}

pub fn plan_runtime_state_cleanup_v1(
    runtime_root: &Path,
    expected_active_bundle_digest: Option<&str>,
    expected_healthy_bundle_digest: Option<&str>,
    bundle_reconciliation_complete: bool,
) -> Result<RuntimeStateCleanupPlanV1, String> {
    validate_optional_digest(expected_active_bundle_digest)?;
    validate_optional_digest(expected_healthy_bundle_digest)?;

    let mut entries = Vec::new();
    if bundle_reconciliation_complete {
        for relative_path in LEGACY_RUNTIME_AUTHORITIES {
            if runtime_root.join(relative_path).symlink_metadata().is_ok() {
                entries.push(RuntimeStateCleanupPlanEntryV1 {
                    relative_path: (*relative_path).to_owned(),
                    reason_kind: RuntimeStateCleanupReasonKind::LegacyRuntimeAuthority,
                    evidence_class: RuntimeStateCleanupEvidenceClass::ActiveHealthyUnreachable,
                });
            }
        }
    }

    entries.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    let plan_digest = cleanup_plan_digest(
        expected_active_bundle_digest,
        expected_healthy_bundle_digest,
        &entries,
    );

    Ok(RuntimeStateCleanupPlanV1 {
        schema_id: RUNTIME_STATE_CLEANUP_PLAN_SCHEMA_ID.to_owned(),
        schema_version: RUNTIME_STATE_CLEANUP_PLAN_SCHEMA_VERSION.to_owned(),
        state: RuntimeStateCleanupPlanState::Planned,
        plan_digest,
        expected_active_bundle_digest: expected_active_bundle_digest.map(str::to_owned),
        expected_healthy_bundle_digest: expected_healthy_bundle_digest.map(str::to_owned),
        entries,
    })
}

pub fn commit_runtime_state_cleanup_v1(
    runtime_root: &Path,
    plan: &RuntimeStateCleanupPlanV1,
    observed_active_bundle_digest: Option<&str>,
    observed_healthy_bundle_digest: Option<&str>,
    cancelled: bool,
) -> Result<RuntimeStateCleanupCommitReceiptV1, String> {
    validate_plan(plan)?;
    if cancelled {
        return Ok(cleanup_terminal(
            plan,
            RuntimeStateCleanupPlanState::Cancelled,
            "runtime-state-cleanup-cancelled",
            0,
        ));
    }
    if plan.expected_active_bundle_digest.as_deref() != observed_active_bundle_digest
        || plan.expected_healthy_bundle_digest.as_deref() != observed_healthy_bundle_digest
    {
        return Err(
            "reasonKind=runtime-state-cleanup-selector-drift cleanup plan no longer binds active and healthy bundles"
                .to_owned(),
        );
    }

    let mut targets = Vec::with_capacity(plan.entries.len());
    for entry in &plan.entries {
        if !LEGACY_RUNTIME_AUTHORITIES.contains(&entry.relative_path.as_str())
            || entry.relative_path.contains('/')
            || entry.relative_path.contains("..")
        {
            return Err(format!(
                "reasonKind=runtime-state-cleanup-path-not-authorized relativePath={}",
                entry.relative_path
            ));
        }
        if entry.reason_kind != RuntimeStateCleanupReasonKind::LegacyRuntimeAuthority
            || entry.evidence_class != RuntimeStateCleanupEvidenceClass::ActiveHealthyUnreachable
        {
            return Err(format!(
                "reasonKind=runtime-state-cleanup-evidence-invalid relativePath={}",
                entry.relative_path
            ));
        }
        targets.push(runtime_root.join(&entry.relative_path));
    }

    let mut deleted_count = 0;
    for target in targets {
        let metadata = match target.symlink_metadata() {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(format!(
                    "reasonKind=runtime-state-cleanup-metadata-failed path={} error={error}",
                    target.display()
                ));
            }
        };
        let result = if metadata.file_type().is_symlink() || metadata.is_file() {
            std::fs::remove_file(&target)
        } else if metadata.is_dir() {
            std::fs::remove_dir_all(&target)
        } else {
            return Err(format!(
                "reasonKind=runtime-state-cleanup-file-type-not-authorized path={}",
                target.display()
            ));
        };
        result.map_err(|error| {
            format!(
                "reasonKind=runtime-state-cleanup-delete-failed path={} error={error}",
                target.display()
            )
        })?;
        deleted_count += 1;
    }

    Ok(cleanup_terminal(
        plan,
        RuntimeStateCleanupPlanState::Committed,
        "runtime-state-cleanup-committed",
        deleted_count,
    ))
}

fn validate_plan(plan: &RuntimeStateCleanupPlanV1) -> Result<(), String> {
    if plan.schema_id != RUNTIME_STATE_CLEANUP_PLAN_SCHEMA_ID
        || plan.schema_version != RUNTIME_STATE_CLEANUP_PLAN_SCHEMA_VERSION
        || plan.state != RuntimeStateCleanupPlanState::Planned
    {
        return Err("reasonKind=runtime-state-cleanup-plan-invalid".to_owned());
    }
    validate_optional_digest(plan.expected_active_bundle_digest.as_deref())?;
    validate_optional_digest(plan.expected_healthy_bundle_digest.as_deref())?;
    let expected = cleanup_plan_digest(
        plan.expected_active_bundle_digest.as_deref(),
        plan.expected_healthy_bundle_digest.as_deref(),
        &plan.entries,
    );
    if plan.plan_digest != expected {
        return Err("reasonKind=runtime-state-cleanup-plan-digest-mismatch".to_owned());
    }
    Ok(())
}

fn cleanup_terminal(
    plan: &RuntimeStateCleanupPlanV1,
    state: RuntimeStateCleanupPlanState,
    reason_kind: &str,
    deleted_count: usize,
) -> RuntimeStateCleanupCommitReceiptV1 {
    RuntimeStateCleanupCommitReceiptV1 {
        schema_id: RUNTIME_STATE_CLEANUP_COMMIT_RECEIPT_SCHEMA_ID.to_owned(),
        schema_version: RUNTIME_STATE_CLEANUP_PLAN_SCHEMA_VERSION.to_owned(),
        state,
        plan_digest: plan.plan_digest.clone(),
        reason_kind: reason_kind.to_owned(),
        deleted_count,
    }
}

fn validate_optional_digest(digest: Option<&str>) -> Result<(), String> {
    let Some(digest) = digest else {
        return Ok(());
    };
    let Some(hex) = digest.strip_prefix("blake3-256:") else {
        return Err("runtime cleanup bundle digest must use blake3-256".to_owned());
    };
    if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("runtime cleanup bundle digest is malformed".to_owned());
    }
    Ok(())
}

fn cleanup_plan_digest(
    active: Option<&str>,
    healthy: Option<&str>,
    entries: &[RuntimeStateCleanupPlanEntryV1],
) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"asp.runtime-state-cleanup-plan.v1\0");
    hash_optional(&mut hasher, active);
    hash_optional(&mut hasher, healthy);
    for entry in entries {
        hasher.update(entry.relative_path.as_bytes());
        hasher.update(b"\0");
        hasher.update(format!("{:?}", entry.reason_kind).as_bytes());
        hasher.update(b"\0");
        hasher.update(format!("{:?}", entry.evidence_class).as_bytes());
        hasher.update(b"\0");
    }
    format!("blake3-256:{}", hasher.finalize().to_hex())
}

fn hash_optional(hasher: &mut blake3::Hasher, value: Option<&str>) {
    match value {
        Some(value) => {
            hasher.update(b"some\0");
            hasher.update(value.as_bytes());
        }
        None => {
            hasher.update(b"none");
        }
    }
    hasher.update(b"\0");
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACTIVE: &str =
        "blake3-256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const HEALTHY: &str =
        "blake3-256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    #[test]
    fn unreconciled_runtime_never_plans_legacy_deletion() {
        let root = tempfile::tempdir().expect("runtime root");
        std::fs::create_dir(root.path().join("resident")).expect("legacy resident");

        let plan =
            plan_runtime_state_cleanup_v1(root.path(), None, None, false).expect("cleanup plan");

        assert!(plan.entries.is_empty());
    }

    #[test]
    fn reconciled_runtime_plans_only_known_legacy_authorities() {
        let root = tempfile::tempdir().expect("runtime root");
        for path in [
            "resident",
            "profiles",
            "bin",
            "provider-locks",
            "artifacts",
            "server",
        ] {
            std::fs::create_dir(root.path().join(path)).expect("runtime path");
        }
        std::fs::write(root.path().join("provider-catalog.v1.json"), b"{}")
            .expect("legacy provider catalog");

        let plan = plan_runtime_state_cleanup_v1(root.path(), Some(ACTIVE), Some(HEALTHY), true)
            .expect("cleanup plan");

        assert_eq!(
            plan.entries
                .iter()
                .map(|entry| entry.relative_path.as_str())
                .collect::<Vec<_>>(),
            [
                "bin",
                "profiles",
                "provider-catalog.v1.json",
                "provider-locks",
                "resident",
            ]
        );
    }

    #[test]
    fn plan_identity_binds_active_and_healthy_bundle_digests() {
        let root = tempfile::tempdir().expect("runtime root");
        let first = plan_runtime_state_cleanup_v1(root.path(), Some(ACTIVE), Some(HEALTHY), true)
            .expect("first plan");
        let second = plan_runtime_state_cleanup_v1(root.path(), Some(HEALTHY), Some(ACTIVE), true)
            .expect("second plan");

        assert_ne!(first.plan_digest, second.plan_digest);
        assert_eq!(first.expected_active_bundle_digest.as_deref(), Some(ACTIVE));
        assert_eq!(
            first.expected_healthy_bundle_digest.as_deref(),
            Some(HEALTHY)
        );
    }

    #[test]
    fn activation_generation_is_not_cleanup_identity() {
        let root = tempfile::tempdir().expect("runtime root");
        let plan = plan_runtime_state_cleanup_v1(root.path(), Some(ACTIVE), Some(HEALTHY), true)
            .expect("cleanup plan");
        let encoded = serde_json::to_string(&plan).expect("serialize plan");

        assert!(!encoded.contains("activationGeneration"));
        assert!(!encoded.to_ascii_lowercase().contains("candidate"));
    }

    #[test]
    fn selector_drift_rejects_before_any_deletion() {
        let root = tempfile::tempdir().expect("runtime root");
        std::fs::create_dir(root.path().join("resident")).expect("legacy resident");
        let plan = plan_runtime_state_cleanup_v1(root.path(), Some(ACTIVE), Some(HEALTHY), true)
            .expect("cleanup plan");

        let error =
            commit_runtime_state_cleanup_v1(root.path(), &plan, Some(HEALTHY), Some(ACTIVE), false)
                .expect_err("selector drift must fail closed");

        assert!(error.contains("runtime-state-cleanup-selector-drift"));
        assert!(root.path().join("resident").exists());
    }

    #[test]
    fn cancellation_deletes_nothing() {
        let root = tempfile::tempdir().expect("runtime root");
        std::fs::create_dir(root.path().join("resident")).expect("legacy resident");
        let plan = plan_runtime_state_cleanup_v1(root.path(), Some(ACTIVE), Some(HEALTHY), true)
            .expect("cleanup plan");

        let receipt =
            commit_runtime_state_cleanup_v1(root.path(), &plan, Some(ACTIVE), Some(HEALTHY), true)
                .expect("cancelled receipt");

        assert_eq!(receipt.state, RuntimeStateCleanupPlanState::Cancelled);
        assert_eq!(receipt.deleted_count, 0);
        assert!(root.path().join("resident").exists());
    }

    #[test]
    fn commit_removes_legacy_authorities_but_preserves_canonical_and_operational_state() {
        let root = tempfile::tempdir().expect("runtime root");
        for path in ["resident", "profiles", "artifacts", "server", "live-corpus"] {
            std::fs::create_dir(root.path().join(path)).expect("runtime path");
        }
        let plan = plan_runtime_state_cleanup_v1(root.path(), Some(ACTIVE), Some(HEALTHY), true)
            .expect("cleanup plan");

        let receipt =
            commit_runtime_state_cleanup_v1(root.path(), &plan, Some(ACTIVE), Some(HEALTHY), false)
                .expect("cleanup commit");

        assert_eq!(receipt.state, RuntimeStateCleanupPlanState::Committed);
        assert_eq!(receipt.deleted_count, 2);
        assert!(!root.path().join("resident").exists());
        assert!(!root.path().join("profiles").exists());
        assert!(root.path().join("artifacts").exists());
        assert!(root.path().join("server").exists());
        assert!(root.path().join("live-corpus").exists());
    }

    #[test]
    fn tampered_plan_path_is_rejected_before_deletion() {
        let root = tempfile::tempdir().expect("runtime root");
        let protected = root.path().join("server");
        std::fs::create_dir(&protected).expect("protected server state");
        let mut plan =
            plan_runtime_state_cleanup_v1(root.path(), Some(ACTIVE), Some(HEALTHY), true)
                .expect("cleanup plan");
        plan.entries.push(RuntimeStateCleanupPlanEntryV1 {
            relative_path: "../server".to_owned(),
            reason_kind: RuntimeStateCleanupReasonKind::LegacyRuntimeAuthority,
            evidence_class: RuntimeStateCleanupEvidenceClass::ActiveHealthyUnreachable,
        });
        plan.plan_digest = cleanup_plan_digest(
            plan.expected_active_bundle_digest.as_deref(),
            plan.expected_healthy_bundle_digest.as_deref(),
            &plan.entries,
        );

        let error =
            commit_runtime_state_cleanup_v1(root.path(), &plan, Some(ACTIVE), Some(HEALTHY), false)
                .expect_err("path escape must fail closed");

        assert!(error.contains("runtime-state-cleanup-path-not-authorized"));
        assert!(protected.exists());
    }
}

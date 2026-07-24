use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyBaselinePackage {
    pub name: String,
    pub version: String,
    pub source_contains: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DownstreamPolicyReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub gate_label: String,
    pub package: ReceiptPackage,
    pub source_snapshot: SourceSnapshot,
    pub policy_digest: String,
    pub execution_command_digest: String,
    pub cache_payload_digest: String,
    pub dependency_baseline_packages: Vec<DependencyBaselinePackage>,
    pub active_verification_task_count: u64,
    pub performance_task_count: u64,
    pub stability_task_count: u64,
    pub performance_report_obligation: bool,
    pub stability_report_obligation: bool,
    pub report_obligations: Vec<ReportObligation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiptPackage {
    pub name: String,
    pub directory: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceSnapshot {
    pub digest: String,
    pub file_count: u64,
    pub byte_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportObligation {
    pub key: String,
    pub renderer: String,
    pub suggested_artifact_name: String,
    pub reason: String,
    pub task_kinds: Vec<String>,
    pub task_fingerprints: Vec<String>,
}

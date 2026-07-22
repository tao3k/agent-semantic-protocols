//! V1 artifact contracts shared by the Rust policy runner and lightweight verifier.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MEMBER_POLICY_REGISTRY_SCHEMA_ID: &str =
    "rust-lang-project-harness.member-policy-registry";
pub const DOWNSTREAM_POLICY_RECEIPT_SCHEMA_ID: &str =
    "rust-lang-project-harness.downstream-policy-receipt";
pub const SCHEMA_VERSION: &str = "1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemberPolicyRegistry {
    pub schema_id: String,
    pub schema_version: String,
    pub harness_execution: HarnessExecution,
    pub members: Vec<MemberPolicy>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HarnessExecution {
    pub runner_package: String,
    pub runner_version: String,
    pub prepare_command: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemberPolicy {
    pub package: String,
    pub package_directory: String,
    pub gate_label: String,
    pub snapshot_excludes: Vec<String>,
    pub agent_advice_allow_explanation: Option<String>,
    pub cargo_check_advice_allow_explanation: Option<String>,
    pub cargo_test_advice_allow_explanation: Option<String>,
    pub rule_severity_overrides: Vec<RuleSeverityOverride>,
    pub verification_skills: Vec<VerificationSkill>,
    pub owners: Vec<OwnerPolicy>,
    pub dependency_baseline_packages: Vec<DependencyBaselinePackage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleSeverityOverride {
    pub rule_id: String,
    pub severity: RuleSeverity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationSkill {
    pub task_kind: TaskKind,
    pub skill_id: String,
    pub adapter: Option<String>,
    pub tool: String,
    pub command: String,
    pub standard: String,
    pub required_inputs: Vec<String>,
    pub pass_criteria: Vec<String>,
    pub receipt_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerPolicy {
    pub owner_path: String,
    pub responsibilities: Vec<OwnerResponsibility>,
    pub task_kinds: Vec<TaskKind>,
    pub task_contracts: Vec<TaskContract>,
    pub stability_picture: Option<StabilityPicture>,
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerResponsibility {
    PureDomainLogic,
    PublicApi,
    ExternalDependency,
    Persistence,
    SecurityBoundary,
    LatencySensitive,
    AvailabilityCritical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskContract {
    pub task_kind: TaskKind,
    pub phase: TaskPhase,
    pub required_receipt: String,
    pub required_evidence: Vec<VerificationRequirement>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskPhase {
    AfterUnitTestsPass,
    BeforeRelease,
    ScheduledRegression,
    BeforeVerification,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationRequirement {
    pub key: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StabilityPicture {
    pub require_long_running_simulation: bool,
    pub require_performance_interface: bool,
    pub require_resource_delta: bool,
    pub require_state_growth: bool,
    pub require_determinism: bool,
    pub require_stability_artifact: bool,
    pub min_iterations: Option<u64>,
    pub min_duration_seconds: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    Stress,
    Performance,
    Stability,
    Chaos,
    Security,
    Regression,
    ResponsibilityReview,
}

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

pub fn validate_registry_identity(registry: &MemberPolicyRegistry) -> Result<(), String> {
    if registry.schema_id != MEMBER_POLICY_REGISTRY_SCHEMA_ID
        || registry.schema_version != SCHEMA_VERSION
    {
        return Err("invalid Rust member-policy registry v1 identity".to_string());
    }
    if registry.members.is_empty() {
        return Err("Rust member-policy registry v1 has no members".to_string());
    }
    for (index, member) in registry.members.iter().enumerate() {
        if member.package.is_empty()
            || member.package_directory.is_empty()
            || member.gate_label.is_empty()
        {
            return Err(format!(
                "invalid empty member identity at registry index {index}"
            ));
        }
        if registry.members[..index]
            .iter()
            .any(|candidate| candidate.package == member.package)
        {
            return Err(format!("duplicate Rust member policy `{}`", member.package));
        }
    }
    Ok(())
}

pub fn validate_receipt_identity(receipt: &DownstreamPolicyReceipt) -> Result<(), String> {
    if receipt.schema_id != DOWNSTREAM_POLICY_RECEIPT_SCHEMA_ID
        || receipt.schema_version != SCHEMA_VERSION
    {
        return Err("invalid Rust downstream-policy receipt v1 identity".to_string());
    }
    for (label, digest) in [
        (
            "source_snapshot.digest",
            receipt.source_snapshot.digest.as_str(),
        ),
        ("policy_digest", receipt.policy_digest.as_str()),
        (
            "execution_command_digest",
            receipt.execution_command_digest.as_str(),
        ),
        (
            "cache_payload_digest",
            receipt.cache_payload_digest.as_str(),
        ),
    ] {
        validate_blake3_digest(label, digest)?;
    }
    Ok(())
}

pub fn canonical_json_digest<T: Serialize>(value: &T) -> Result<String, String> {
    let value = serde_json::to_value(value).map_err(|error| error.to_string())?;
    let mut canonical = String::new();
    write_canonical_json(&value, &mut canonical)?;
    Ok(format!(
        "blake3:{}",
        blake3::hash(canonical.as_bytes()).to_hex()
    ))
}

fn validate_blake3_digest(label: &str, digest: &str) -> Result<(), String> {
    let Some(hex) = digest.strip_prefix("blake3:") else {
        return Err(format!("{label} is not a blake3 digest"));
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("{label} is not lowercase blake3 hex"));
    }
    Ok(())
}

fn write_canonical_json(value: &Value, output: &mut String) -> Result<(), String> {
    match value {
        Value::Null => output.push_str("null"),
        Value::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
        Value::Number(value) => output.push_str(&value.to_string()),
        Value::String(value) => {
            output.push_str(&serde_json::to_string(value).map_err(|error| error.to_string())?)
        }
        Value::Array(values) => {
            output.push('[');
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.push(',');
                }
                write_canonical_json(value, output)?;
            }
            output.push(']');
        }
        Value::Object(values) => {
            output.push('{');
            let mut keys = values.keys().collect::<Vec<_>>();
            keys.sort_unstable();
            for (index, key) in keys.into_iter().enumerate() {
                if index != 0 {
                    output.push(',');
                }
                output.push_str(&serde_json::to_string(key).map_err(|error| error.to_string())?);
                output.push(':');
                write_canonical_json(&values[key], output)?;
            }
            output.push('}');
        }
    }
    Ok(())
}

use serde::{Deserialize, Serialize};

use crate::{DependencyBaselinePackage, OwnerPolicy, VerificationSkill};

pub const MEMBER_POLICY_REGISTRY_SCHEMA_ID: &str =
    "rust-lang-project-harness.member-policy-registry";
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

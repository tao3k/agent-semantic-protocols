use serde::{Deserialize, Serialize};

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

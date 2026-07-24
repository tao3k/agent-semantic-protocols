//! V1 artifact contracts shared by the Rust policy runner and lightweight verifier.

mod canonical_json;
mod canonical_json_digest;
mod receipt;
mod receipt_schema;
mod registry;
mod validation;
mod verification;

pub use canonical_json_digest::canonical_json_digest;
pub use receipt::{
    DependencyBaselinePackage, DownstreamPolicyReceipt, ReceiptPackage, ReportObligation,
    SourceSnapshot,
};
pub use receipt_schema::DOWNSTREAM_POLICY_RECEIPT_SCHEMA_ID;
pub use registry::{
    HarnessExecution, MEMBER_POLICY_REGISTRY_SCHEMA_ID, MemberPolicy, MemberPolicyRegistry,
    RuleSeverity, RuleSeverityOverride, SCHEMA_VERSION,
};
pub use validation::{validate_receipt_identity, validate_registry_identity};
pub use verification::{
    OwnerPolicy, OwnerResponsibility, StabilityPicture, TaskContract, TaskKind, TaskPhase,
    VerificationRequirement, VerificationSkill,
};

mod error;
mod model;
mod primitives;
mod protocol;
mod validation;

pub use error::ValidationError;

pub use model::{
    ActiveProgram, CONTEXT_PRODUCT_CANONICALIZATION_PROFILE, ClaimClass, ClosureDisposition,
    ContextBinding, DecisionRequirement, EvidenceVerdict, ExecutionAuthority, FrontierAntichain,
    FrontierNode, JSON_SAFE_INTEGER_MAX, Obligation, ObligationDisposition, ProofReuse,
    ProofReuseMode, RetainedProof, SearchBudget, UncheckedContextProductStateV1,
};
pub use model::{CONTEXT_PRODUCT_SCHEMA_ID, CONTEXT_PRODUCT_SCHEMA_VERSION};
pub use primitives::{Digest, ProtocolId};
pub use protocol::{
    ActionAdmitted, ActionAdmittedEventType, ClosureFinalizedEventType, ClosureProof,
    ContextProductEvent, EffectClass, EvidenceCompleteness, EvidencePredicate, EvidenceReceipt,
    EvidenceScope, ExecutionConsumed, ExecutionConsumedEventType, ExecutionGrantIssued,
    ExecutionGrantIssuedEventType, ExecutionRevoked, ExecutionRevokedEventType, ExecutionStarted,
    ExecutionStartedEventType, JoinPolicy, ParserOwnedCommandAdmission, RecommendedNextAdmission,
    RecommendedNextCandidate, RouteActionClass, RouteEdge, RouteJoin, RouteNode, RouteProgram,
    RouteProgramAdmitted, RouteProgramAdmittedEventType, RouteProposal, RouteStage,
    SearchClosureReceipt, StateAuthorityReceipt,
};
pub use validation::chained_event_log_digest;

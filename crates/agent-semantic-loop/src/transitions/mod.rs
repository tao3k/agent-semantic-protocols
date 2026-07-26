mod admission;
mod closure;
mod execution;
mod grant;
mod join;
mod transition_core;

#[cfg(test)]
pub(super) use transition_core::canonical_digest;

#[cfg(test)]
#[path = "../../tests/unit/transitions.rs"]
mod tests;

pub use admission::{AdmitSearchLoopDirectiveRequest, ExecutionDispatchAdmission};
pub use closure::FinalizeClosureRequest;
pub use execution::{
    ConsumeExecutionGroupRequest, ExecutionConsumptionSpec, ExecutionRevocationSpec,
    ExecutionStartSpec, RevokeExecutionGroupRequest, StartExecutionGroupRequest,
};
pub use grant::{ExecutionGrantSpec, IssueExecutionGroupGrantsRequest};
pub use join::JoinExecutionGroupRequest;

use std::fmt;
use std::future::Future;
use std::pin::Pin;

use agent_semantic_context_product::{
    ContextProductEvent, Digest, EffectClass, EvidenceReceipt, ProtocolId, StateAuthorityReceipt,
    UncheckedContextProductStateV1,
};

pub type PortFuture<'a, T, E> = Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'a>>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateHead {
    pub run_id: ProtocolId,
    pub revision: u64,
    pub state_digest: Digest,
    pub event_log_digest: Digest,
    pub last_event_sequence: u64,
}

#[derive(Clone, Debug)]
pub struct AuthoritativeStateRecord {
    pub state: UncheckedContextProductStateV1,
    pub authority_receipt: StateAuthorityReceipt,
    pub search_loop_capabilities: Vec<crate::search_capability::UncheckedSearchLoopCapabilityV1>,
    pub search_loop_runtime: Option<crate::search_runtime::UncheckedSearchLoopRuntimeBindingV1>,
}

#[derive(Clone, Debug)]
pub struct RunCommit {
    pub(crate) expected: StateHead,
    pub(crate) events: Vec<ContextProductEvent>,
    pub(crate) next_state: UncheckedContextProductStateV1,
    pub(crate) committed_at_ms: u64,
    pub(crate) search_loop_capabilities:
        Vec<crate::search_capability::UncheckedSearchLoopCapabilityV1>,
    pub(crate) search_loop_runtime:
        Option<crate::search_runtime::UncheckedSearchLoopRuntimeBindingV1>,
}

impl RunCommit {
    pub fn expected(&self) -> &StateHead {
        &self.expected
    }

    pub fn events(&self) -> &[ContextProductEvent] {
        &self.events
    }

    pub fn next_state(&self) -> &UncheckedContextProductStateV1 {
        &self.next_state
    }

    pub fn committed_at_ms(&self) -> u64 {
        self.committed_at_ms
    }

    pub fn search_loop_capabilities(
        &self,
    ) -> &[crate::search_capability::UncheckedSearchLoopCapabilityV1] {
        &self.search_loop_capabilities
    }

    pub fn search_loop_runtime(
        &self,
    ) -> Option<&crate::search_runtime::UncheckedSearchLoopRuntimeBindingV1> {
        self.search_loop_runtime.as_ref()
    }
}

#[derive(Clone, Debug)]
pub struct RunCommitReceipt {
    pub authority_receipt: StateAuthorityReceipt,
}

#[derive(Clone, Debug)]
pub enum CompareAndAppendOutcome {
    Committed(RunCommitReceipt),
    Conflict(StateHead),
}

pub trait RunCommitStore: Send + Sync {
    type Error: fmt::Display + Send + Sync + 'static;

    fn load<'a>(
        &'a self,
        run_id: &'a ProtocolId,
    ) -> PortFuture<'a, AuthoritativeStateRecord, Self::Error>;

    fn compare_and_append<'a>(
        &'a self,
        commit: RunCommit,
    ) -> PortFuture<'a, CompareAndAppendOutcome, Self::Error>;
}

pub trait SearchLoopRuntimeStore: RunCommitStore {
    fn load_by_loop_id<'a>(
        &'a self,
        loop_id: &'a ProtocolId,
    ) -> PortFuture<'a, Option<AuthoritativeStateRecord>, Self::Error>;
}

pub trait ProofResolver: Send + Sync {
    type Error: fmt::Display + Send + Sync + 'static;

    fn resolve<'a>(
        &'a self,
        proof_ref: &'a ProtocolId,
    ) -> PortFuture<'a, EvidenceReceipt, Self::Error>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderExecutionDispatch {
    pub execution_group_id: ProtocolId,
    pub stage_ids: Vec<ProtocolId>,
    pub attempt_id: ProtocolId,
    pub grant_id: ProtocolId,
    pub grant_digest: Digest,
    pub provider_id: ProtocolId,
    pub operation: ProtocolId,
    pub resolved_input_digest: Digest,
    pub effect_class: EffectClass,
    pub lease_fence: u64,
    pub expires_at_ms: u64,
    pub provider_idempotency_key: ProtocolId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderExecutionResult {
    pub attempt_id: ProtocolId,
    pub result_receipt_ref: ProtocolId,
    pub result_digest: Digest,
}

pub trait SearchExecutionDriver: Send + Sync {
    type Error: fmt::Display + Send + Sync + 'static;

    fn execute_group<'a>(
        &'a self,
        execution_group_id: &'a ProtocolId,
        dispatches: Vec<ProviderExecutionDispatch>,
    ) -> PortFuture<'a, Vec<ProviderExecutionResult>, Self::Error>;
}

pub trait TrustedClock: Send + Sync {
    fn now_ms(&self) -> u64;
}

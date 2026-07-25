use std::fmt;
use std::future::Future;
use std::pin::Pin;

use agent_semantic_context_product::{
    ContextProductEvent, Digest, EvidenceReceipt, ProtocolId, StateAuthorityReceipt,
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
}

#[derive(Clone, Debug)]
pub struct RunCommit {
    pub(crate) expected: StateHead,
    pub(crate) events: Vec<ContextProductEvent>,
    pub(crate) next_state: UncheckedContextProductStateV1,
    pub(crate) committed_at_ms: u64,
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

pub trait ProofResolver: Send + Sync {
    type Error: fmt::Display + Send + Sync + 'static;

    fn resolve<'a>(
        &'a self,
        proof_ref: &'a ProtocolId,
    ) -> PortFuture<'a, EvidenceReceipt, Self::Error>;
}

pub trait TrustedClock: Send + Sync {
    fn now_ms(&self) -> u64;
}

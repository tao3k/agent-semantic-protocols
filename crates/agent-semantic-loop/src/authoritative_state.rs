use agent_semantic_context_product::{
    ActiveProgram, ClosureDisposition, Digest, ExecutionAuthority, ProtocolId,
    StateAuthorityReceipt, UncheckedContextProductStateV1, ValidationError,
};

use crate::ports::{AuthoritativeStateRecord, StateHead};

#[derive(Clone, Debug)]
pub struct ValidatedContextProductStateV1 {
    state: UncheckedContextProductStateV1,
    authority_receipt: StateAuthorityReceipt,
}

impl ValidatedContextProductStateV1 {
    pub(crate) fn from_authoritative_record(
        record: AuthoritativeStateRecord,
    ) -> Result<Self, ValidationError> {
        record.state.validate()?;
        record.authority_receipt.validate_for_state(&record.state)?;
        Ok(Self {
            state: record.state,
            authority_receipt: record.authority_receipt,
        })
    }

    pub fn run_id(&self) -> &ProtocolId {
        &self.state.run_id
    }

    pub fn revision(&self) -> u64 {
        self.state.revision
    }

    pub fn state_digest(&self) -> &Digest {
        &self.state.state_digest
    }

    pub fn context_binding_digest(&self) -> &Digest {
        &self.state.context.binding_digest
    }

    pub fn active_program(&self) -> &ActiveProgram {
        &self.state.active_program
    }

    pub fn execution(&self) -> &ExecutionAuthority {
        &self.state.execution
    }

    pub fn closure(&self) -> &ClosureDisposition {
        &self.state.closure
    }

    pub fn authority_receipt(&self) -> &StateAuthorityReceipt {
        &self.authority_receipt
    }

    pub fn head(&self) -> StateHead {
        StateHead {
            run_id: self.state.run_id.clone(),
            revision: self.state.revision,
            state_digest: self.state.state_digest.clone(),
            event_log_digest: self.state.event_log_digest.clone(),
            last_event_sequence: self.state.last_event_sequence,
        }
    }

    pub(crate) fn wire(&self) -> &UncheckedContextProductStateV1 {
        &self.state
    }
}

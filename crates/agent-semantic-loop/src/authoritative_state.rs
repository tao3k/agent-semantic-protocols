use agent_semantic_context_product::{
    ActiveProgram, ClosureDisposition, Digest, ExecutionAuthority, ProtocolId,
    StateAuthorityReceipt, UncheckedContextProductStateV1, ValidationError,
};

use crate::ports::{AuthoritativeStateRecord, StateHead};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AuthoritativeStateValidationError {
    ContextProduct(ValidationError),
    SearchLoopCapability(crate::search_capability::SearchLoopCapabilityValidationError),
    SearchLoopRuntime(crate::search_runtime::SearchLoopRuntimeValidationError),
    SearchLoopRuntimeBinding,
}

impl std::fmt::Display for AuthoritativeStateValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ContextProduct(error) => error.fmt(formatter),
            Self::SearchLoopCapability(error) => error.fmt(formatter),
            Self::SearchLoopRuntime(error) => error.fmt(formatter),
            Self::SearchLoopRuntimeBinding => {
                formatter.write_str("search-loop runtime binding disagrees with context state")
            }
        }
    }
}

impl std::error::Error for AuthoritativeStateValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ContextProduct(error) => Some(error),
            Self::SearchLoopCapability(error) => Some(error),
            Self::SearchLoopRuntime(error) => Some(error),
            Self::SearchLoopRuntimeBinding => None,
        }
    }
}

impl From<ValidationError> for AuthoritativeStateValidationError {
    fn from(error: ValidationError) -> Self {
        Self::ContextProduct(error)
    }
}

impl From<crate::search_capability::SearchLoopCapabilityValidationError>
    for AuthoritativeStateValidationError
{
    fn from(error: crate::search_capability::SearchLoopCapabilityValidationError) -> Self {
        Self::SearchLoopCapability(error)
    }
}

#[derive(Clone, Debug)]
pub struct ValidatedContextProductStateV1 {
    state: UncheckedContextProductStateV1,
    authority_receipt: StateAuthorityReceipt,
    search_loop_capabilities: Vec<crate::search_capability::UncheckedSearchLoopCapabilityV1>,
    search_loop_runtime: Option<crate::search_runtime::UncheckedSearchLoopRuntimeBindingV1>,
}

impl ValidatedContextProductStateV1 {
    pub(crate) fn from_authoritative_record(
        record: AuthoritativeStateRecord,
    ) -> Result<Self, AuthoritativeStateValidationError> {
        record.state.validate()?;
        record.authority_receipt.validate_for_state(&record.state)?;
        for capability in &record.search_loop_capabilities {
            crate::search_capability::SearchLoopCapabilityV1::from_unchecked(capability.clone())
                .map_err(AuthoritativeStateValidationError::SearchLoopCapability)?;
        }
        if let Some(runtime) = &record.search_loop_runtime {
            let runtime =
                crate::search_runtime::SearchLoopRuntimeBindingV1::validate(runtime.clone())
                    .map_err(AuthoritativeStateValidationError::SearchLoopRuntime)?;
            if runtime.run_id() != &record.state.run_id
                || runtime.context_binding_digest() != &record.state.context.binding_digest
            {
                return Err(AuthoritativeStateValidationError::SearchLoopRuntimeBinding);
            }
        }
        Ok(Self {
            state: record.state,
            authority_receipt: record.authority_receipt,
            search_loop_capabilities: record.search_loop_capabilities,
            search_loop_runtime: record.search_loop_runtime,
        })
    }

    pub fn search_loop_runtime(
        &self,
    ) -> Option<&crate::search_runtime::UncheckedSearchLoopRuntimeBindingV1> {
        self.search_loop_runtime.as_ref()
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

    pub fn executions(&self) -> &[ExecutionAuthority] {
        &self.state.executions
    }

    pub fn closure(&self) -> &ClosureDisposition {
        &self.state.closure
    }

    pub fn authority_receipt(&self) -> &StateAuthorityReceipt {
        &self.authority_receipt
    }

    pub fn search_loop_capabilities(
        &self,
    ) -> &[crate::search_capability::UncheckedSearchLoopCapabilityV1] {
        &self.search_loop_capabilities
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

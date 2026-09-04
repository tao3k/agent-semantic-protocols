use std::fmt;

use agent_semantic_context_product::ActiveProgram;
use agent_semantic_context_product::ContextProductEvent;
use agent_semantic_context_product::Digest;
use agent_semantic_context_product::JSON_SAFE_INTEGER_MAX;
use agent_semantic_context_product::ProtocolId;
use agent_semantic_context_product::RouteProgram;
use agent_semantic_context_product::RouteProgramAdmitted;
use agent_semantic_context_product::RouteProgramAdmittedEventType;
use agent_semantic_context_product::RouteProposal;
use agent_semantic_context_product::UncheckedContextProductStateV1;
use agent_semantic_context_product::ValidationError;
use agent_semantic_context_product::chained_event_log_digest;

use crate::authoritative_state::AuthoritativeStateValidationError;
use crate::authoritative_state::ValidatedContextProductStateV1;
use crate::ports::AuthoritativeStateRecord;
use crate::ports::CompareAndAppendOutcome;
use crate::ports::ProofResolver;
use crate::ports::RunCommit;
use crate::ports::RunCommitStore;
use crate::ports::StateHead;
use crate::ports::TrustedClock;
use crate::route_validation::RouteValidationError;
use crate::route_validation::validate_route;

#[derive(Clone, Debug)]
pub struct AdmitRouteProgramRequest {
    pub run_id: ProtocolId,
    pub expected_revision: u64,
    pub expected_state_digest: Digest,
    pub expected_context_binding_digest: Digest,
    pub event_id: ProtocolId,
    pub proposal: RouteProposal,
    pub program: RouteProgram,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GraphRouterError {
    Store(String),
    Proof(String),
    Provider(String),
    Capability(crate::search_capability::SearchLoopCapabilityValidationError),
    Runtime(crate::search_runtime::SearchLoopRuntimeValidationError),
    RuntimeBinding,
    MissingSearchLoop(ProtocolId),
    Validation(ValidationError),
    Route(RouteValidationError),
    Conflict(StateHead),
    InvalidTransition(&'static str),
    UnsafeClock(u64),
}

impl fmt::Display for GraphRouterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => write!(formatter, "context run store failed: {error}"),
            Self::Proof(error) => write!(formatter, "context proof resolver failed: {error}"),
            Self::Provider(error) => write!(formatter, "search provider execution failed: {error}"),
            Self::Capability(error) => {
                write!(formatter, "search-loop capability rejected: {error}")
            }
            Self::Runtime(error) => {
                write!(formatter, "search-loop runtime rejected: {error}")
            }
            Self::RuntimeBinding => {
                formatter.write_str("search-loop runtime disagrees with context state")
            }
            Self::MissingSearchLoop(loop_id) => {
                write!(formatter, "search-loop `{loop_id}` is not registered")
            }
            Self::Validation(error) => write!(formatter, "context state rejected: {error}"),
            Self::Route(error) => write!(formatter, "route rejected: {error}"),
            Self::Conflict(head) => write!(
                formatter,
                "context run conflict at revision {} digest {}",
                head.revision, head.state_digest
            ),
            Self::InvalidTransition(reason) => {
                write!(formatter, "context transition rejected: {reason}")
            }
            Self::UnsafeClock(value) => {
                write!(
                    formatter,
                    "trusted clock exceeds JSON safe integer: {value}"
                )
            }
        }
    }
}

impl std::error::Error for GraphRouterError {}

impl From<AuthoritativeStateValidationError> for GraphRouterError {
    fn from(error: AuthoritativeStateValidationError) -> Self {
        match error {
            AuthoritativeStateValidationError::ContextProduct(error) => Self::Validation(error),
            AuthoritativeStateValidationError::SearchLoopCapability(error) => {
                Self::Capability(error)
            }
            AuthoritativeStateValidationError::SearchLoopRuntime(error) => Self::Runtime(error),
            AuthoritativeStateValidationError::SearchLoopRuntimeBinding => Self::RuntimeBinding,
        }
    }
}

pub struct GraphRouter<S, P, C> {
    pub(crate) store: S,
    pub(crate) proof_resolver: P,
    pub(crate) clock: C,
}

impl<S, P, C> GraphRouter<S, P, C>
where
    S: RunCommitStore,
    P: ProofResolver,
    C: TrustedClock,
{
    pub fn new(store: S, proof_resolver: P, clock: C) -> Self {
        Self {
            store,
            proof_resolver,
            clock,
        }
    }

    pub async fn load(
        &self,
        run_id: &ProtocolId,
    ) -> Result<ValidatedContextProductStateV1, GraphRouterError> {
        let now_ms = self.clock.now_ms();
        if now_ms > JSON_SAFE_INTEGER_MAX {
            return Err(GraphRouterError::UnsafeClock(now_ms));
        }
        let record = self
            .store
            .load(run_id)
            .await
            .map_err(|error| GraphRouterError::Store(error.to_string()))?;
        ValidatedContextProductStateV1::from_authoritative_record(record)
            .map_err(GraphRouterError::from)
    }

    pub async fn load_search_loop(
        &self,
        loop_id: &ProtocolId,
    ) -> Result<ValidatedContextProductStateV1, GraphRouterError>
    where
        S: crate::ports::SearchLoopRuntimeStore,
    {
        let now_ms = self.clock.now_ms();
        if now_ms > JSON_SAFE_INTEGER_MAX {
            return Err(GraphRouterError::UnsafeClock(now_ms));
        }
        let record = self
            .store
            .load_by_loop_id(loop_id)
            .await
            .map_err(|error| GraphRouterError::Store(error.to_string()))?
            .ok_or_else(|| GraphRouterError::MissingSearchLoop(loop_id.clone()))?;
        ValidatedContextProductStateV1::from_authoritative_record(record)
            .map_err(GraphRouterError::from)
    }

    pub async fn admit_route_program(
        &self,
        request: AdmitRouteProgramRequest,
    ) -> Result<ValidatedContextProductStateV1, GraphRouterError> {
        self.admit_route_program_inner(request, None).await
    }

    pub async fn admit_route_program_with_capability_spend(
        &self,
        request: AdmitRouteProgramRequest,
        capability_spend: crate::search_capability::SearchLoopCapabilitySpend,
    ) -> Result<ValidatedContextProductStateV1, GraphRouterError> {
        self.admit_route_program_inner(request, Some(capability_spend))
            .await
    }

    async fn admit_route_program_inner(
        &self,
        request: AdmitRouteProgramRequest,
        capability_spend: Option<crate::search_capability::SearchLoopCapabilitySpend>,
    ) -> Result<ValidatedContextProductStateV1, GraphRouterError> {
        let current = self.load(&request.run_id).await?;
        let committed_at_ms = self.clock.now_ms();
        if committed_at_ms > JSON_SAFE_INTEGER_MAX {
            return Err(GraphRouterError::UnsafeClock(committed_at_ms));
        }
        require_expected_head(&current, &request)?;
        if !matches!(current.active_program(), ActiveProgram::None)
            || !current.executions().is_empty()
        {
            return Err(GraphRouterError::InvalidTransition(
                "route program requires no active program or execution authority",
            ));
        }
        validate_route(current.wire(), &request.proposal, &request.program)
            .map_err(GraphRouterError::Route)?;

        let next_revision = current
            .revision()
            .checked_add(1)
            .filter(|revision| *revision <= JSON_SAFE_INTEGER_MAX)
            .ok_or(GraphRouterError::Validation(
                ValidationError::RevisionExhausted,
            ))?;
        let next_sequence = current
            .wire()
            .last_event_sequence
            .checked_add(1)
            .filter(|sequence| *sequence <= JSON_SAFE_INTEGER_MAX)
            .ok_or(GraphRouterError::Validation(
                ValidationError::RevisionExhausted,
            ))?;
        let mut event = RouteProgramAdmitted {
            event_type: RouteProgramAdmittedEventType::RouteProgramAdmitted,
            event_id: request.event_id,
            run_id: request.run_id,
            sequence: next_sequence,
            state_revision: current.revision(),
            pre_state_digest: current.state_digest().clone(),
            proposal_digest: request.proposal.proposal_digest.clone(),
            program_digest: request.program.program_digest.clone(),
            context_binding_digest: current.context_binding_digest().clone(),
            event_digest: Digest::from_bytes(b"pending-route-program-event"),
        };
        event.event_digest = event.recompute_event_digest();
        let next_event_log_digest =
            chained_event_log_digest(&current.wire().event_log_digest, &event.event_digest);
        let authority_receipt_ref =
            authority_receipt_id(&current.head(), next_revision, &next_event_log_digest)?;

        let mut next_state: UncheckedContextProductStateV1 = current.wire().clone();
        next_state.revision = next_revision;
        next_state.previous_state_digest = Some(current.state_digest().clone());
        next_state.last_event_sequence = next_sequence;
        next_state.event_log_digest = next_event_log_digest;
        next_state.active_program = ActiveProgram::Admitted {
            proposal_id: request.proposal.proposal_id,
            proposal_digest: request.proposal.proposal_digest,
            intent_digest: request.proposal.intent_digest,
            program: Box::new(request.program.clone()),
            program_id: request.program.program_id,
            program_digest: request.program.program_digest,
            graph_digest: request.program.graph_digest,
            admitted_at_revision: current.revision(),
            context_binding_digest: current.context_binding_digest().clone(),
        };
        next_state.authority_receipt_ref = authority_receipt_ref;
        next_state.state_digest = next_state.recompute_state_digest();
        next_state
            .validate()
            .map_err(GraphRouterError::Validation)?;
        let capability_mutations = capability_spend
            .map(|spend| {
                crate::search_capability::SearchLoopCapabilityMutation::Consume(
                    crate::search_capability::SearchLoopCapabilityConsumption::new(
                        spend.capability_id().clone(),
                        spend.token_digest().clone(),
                        next_revision,
                        next_state.authority_receipt_ref.clone(),
                    ),
                )
            })
            .into_iter()
            .collect::<Vec<_>>();
        let mut search_loop_capabilities = current.search_loop_capabilities().to_vec();
        crate::search_capability::apply_capability_mutations(
            &mut search_loop_capabilities,
            &capability_mutations,
        )
        .map_err(|error| GraphRouterError::Store(error.to_string()))?;

        let expected = current.head();
        let outcome = self
            .store
            .compare_and_append(RunCommit {
                expected,
                events: vec![ContextProductEvent::RouteProgramAdmitted(event)],
                next_state: next_state.clone(),
                committed_at_ms,
                search_loop_capabilities: search_loop_capabilities.clone(),
                search_loop_runtime: current.search_loop_runtime().cloned(),
            })
            .await
            .map_err(|error| GraphRouterError::Store(error.to_string()))?;
        match outcome {
            CompareAndAppendOutcome::Committed(receipt) => {
                ValidatedContextProductStateV1::from_authoritative_record(
                    AuthoritativeStateRecord {
                        state: next_state,
                        authority_receipt: receipt.authority_receipt,
                        search_loop_capabilities,
                        search_loop_runtime: current.search_loop_runtime().cloned(),
                    },
                )
                .map_err(GraphRouterError::from)
            }
            CompareAndAppendOutcome::Conflict(head) => Err(GraphRouterError::Conflict(head)),
        }
    }

    pub fn proof_resolver(&self) -> &P {
        &self.proof_resolver
    }
}

fn require_expected_head(
    current: &ValidatedContextProductStateV1,
    request: &AdmitRouteProgramRequest,
) -> Result<(), GraphRouterError> {
    if current.revision() != request.expected_revision
        || current.state_digest() != &request.expected_state_digest
        || current.context_binding_digest() != &request.expected_context_binding_digest
    {
        return Err(GraphRouterError::Conflict(current.head()));
    }
    Ok(())
}

pub(crate) fn authority_receipt_id(
    current: &StateHead,
    next_revision: u64,
    next_event_log_digest: &Digest,
) -> Result<ProtocolId, GraphRouterError> {
    let seed = serde_json::to_vec(&serde_json::json!({
        "eventLogDigest": next_event_log_digest,
        "nextRevision": next_revision,
        "preStateDigest": current.state_digest,
        "runId": current.run_id,
    }))
    .expect("authority receipt seed serializes");
    let digest = Digest::from_bytes(&seed);
    ProtocolId::parse(format!(
        "authority-receipt:{}",
        digest.as_str().trim_start_matches("blake3:")
    ))
    .map_err(GraphRouterError::Validation)
}

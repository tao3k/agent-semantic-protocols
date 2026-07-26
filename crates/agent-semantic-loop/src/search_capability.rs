use std::fmt;

use agent_semantic_context_product::{Digest, ProtocolId};

use crate::ValidatedContextProductStateV1;
use serde::{Deserialize, Serialize};

pub const SEARCH_LOOP_CAPABILITY_SCHEMA_ID: &str =
    "agent.semantic-protocols.search-loop-capability";
pub const SEARCH_LOOP_CAPABILITY_SCHEMA_VERSION: &str = "1";
const SEARCH_LOOP_CAPABILITY_TOKEN_PREFIX: &str = "capability.";
const SEARCH_LOOP_CAPABILITY_TOKEN_ENTROPY_BYTES: usize = 32;
const LOWER_HEX: &[u8; 16] = b"0123456789abcdef";

#[derive(Clone, Eq, PartialEq)]
pub struct SearchLoopCapabilityToken(String);

impl SearchLoopCapabilityToken {
    pub fn generate() -> Result<Self, SearchLoopCapabilityTokenGenerationError> {
        let mut entropy = [0_u8; SEARCH_LOOP_CAPABILITY_TOKEN_ENTROPY_BYTES];
        getrandom::fill(&mut entropy)
            .map_err(SearchLoopCapabilityTokenGenerationError::EntropyUnavailable)?;

        let mut value = String::with_capacity(
            SEARCH_LOOP_CAPABILITY_TOKEN_PREFIX.len()
                + SEARCH_LOOP_CAPABILITY_TOKEN_ENTROPY_BYTES * 2,
        );
        value.push_str(SEARCH_LOOP_CAPABILITY_TOKEN_PREFIX);
        for byte in entropy {
            value.push(LOWER_HEX[usize::from(byte >> 4)] as char);
            value.push(LOWER_HEX[usize::from(byte & 0x0f)] as char);
        }
        Ok(Self(value))
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, SearchLoopCapabilityValidationError> {
        let value = value.into();
        let valid_payload = value
            .strip_prefix(SEARCH_LOOP_CAPABILITY_TOKEN_PREFIX)
            .is_some_and(|payload| {
                payload.len() == SEARCH_LOOP_CAPABILITY_TOKEN_ENTROPY_BYTES * 2
                    && payload
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
            });
        if valid_payload {
            Ok(Self(value))
        } else {
            Err(SearchLoopCapabilityValidationError::InvalidToken)
        }
    }

    pub fn expose_secret(&self) -> &str {
        &self.0
    }

    pub fn digest(&self) -> Digest {
        Digest::from_bytes(self.0.as_bytes())
    }
}

#[derive(Debug)]
pub enum SearchLoopCapabilityTokenGenerationError {
    EntropyUnavailable(getrandom::Error),
}

impl fmt::Display for SearchLoopCapabilityTokenGenerationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EntropyUnavailable(_) => {
                formatter.write_str("operating-system entropy unavailable for capability issuance")
            }
        }
    }
}

impl std::error::Error for SearchLoopCapabilityTokenGenerationError {}

impl fmt::Debug for SearchLoopCapabilityToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SearchLoopCapabilityToken([REDACTED])")
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SearchLoopCapabilityCommand {
    #[serde(rename = "asp.search.loop.advance.v1")]
    Advance,
    #[serde(rename = "asp.search.loop.poll.v1")]
    Poll,
    #[serde(rename = "asp.search.loop.receipt.v1")]
    Receipt,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SearchLoopCapabilityReplayPolicy {
    OneShot,
    StateBoundIdempotent,
    TerminalIdempotent,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SearchLoopCapabilityStatus {
    Active,
    Consumed,
    Revoked,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchLoopCapabilityStateBinding {
    run_id: ProtocolId,
    revision: u64,
    state_digest: Digest,
    authority_receipt_ref: ProtocolId,
}

impl SearchLoopCapabilityStateBinding {
    pub fn new(
        run_id: ProtocolId,
        revision: u64,
        state_digest: Digest,
        authority_receipt_ref: ProtocolId,
    ) -> Self {
        Self {
            run_id,
            revision,
            state_digest,
            authority_receipt_ref,
        }
    }

    pub fn run_id(&self) -> &ProtocolId {
        &self.run_id
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn state_digest(&self) -> &Digest {
        &self.state_digest
    }

    pub fn authority_receipt_ref(&self) -> &ProtocolId {
        &self.authority_receipt_ref
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdvanceCapabilitySubject {
    kind: AdvanceCapabilitySubjectKind,
    panel_id: ProtocolId,
    choice_id: ProtocolId,
    proposal_ref: ProtocolId,
    proposal_digest: Digest,
    execution_group_ref: ProtocolId,
}

impl AdvanceCapabilitySubject {
    pub fn new(
        panel_id: ProtocolId,
        choice_id: ProtocolId,
        proposal_ref: ProtocolId,
        proposal_digest: Digest,
        execution_group_ref: ProtocolId,
    ) -> Self {
        Self {
            kind: AdvanceCapabilitySubjectKind::Advance,
            panel_id,
            choice_id,
            proposal_ref,
            proposal_digest,
            execution_group_ref,
        }
    }

    pub fn panel_id(&self) -> &ProtocolId {
        &self.panel_id
    }

    pub fn choice_id(&self) -> &ProtocolId {
        &self.choice_id
    }

    pub fn proposal_ref(&self) -> &ProtocolId {
        &self.proposal_ref
    }

    pub fn proposal_digest(&self) -> &Digest {
        &self.proposal_digest
    }

    pub fn execution_group_ref(&self) -> &ProtocolId {
        &self.execution_group_ref
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum AdvanceCapabilitySubjectKind {
    #[serde(rename = "advance")]
    Advance,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PollCapabilitySubject {
    kind: PollCapabilitySubjectKind,
    batch_id: ProtocolId,
    execution_group_ref: ProtocolId,
}

impl PollCapabilitySubject {
    pub fn new(batch_id: ProtocolId, execution_group_ref: ProtocolId) -> Self {
        Self {
            kind: PollCapabilitySubjectKind::Poll,
            batch_id,
            execution_group_ref,
        }
    }

    pub fn batch_id(&self) -> &ProtocolId {
        &self.batch_id
    }

    pub fn execution_group_ref(&self) -> &ProtocolId {
        &self.execution_group_ref
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum PollCapabilitySubjectKind {
    #[serde(rename = "poll")]
    Poll,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReceiptCapabilitySubject {
    kind: ReceiptCapabilitySubjectKind,
    closure_receipt_ref: ProtocolId,
    closure_receipt_digest: Digest,
}

impl ReceiptCapabilitySubject {
    pub fn new(closure_receipt_ref: ProtocolId, closure_receipt_digest: Digest) -> Self {
        Self {
            kind: ReceiptCapabilitySubjectKind::Receipt,
            closure_receipt_ref,
            closure_receipt_digest,
        }
    }

    pub fn closure_receipt_ref(&self) -> &ProtocolId {
        &self.closure_receipt_ref
    }

    pub fn closure_receipt_digest(&self) -> &Digest {
        &self.closure_receipt_digest
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum ReceiptCapabilitySubjectKind {
    #[serde(rename = "receipt")]
    Receipt,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum SearchLoopCapabilitySubject {
    Advance(AdvanceCapabilitySubject),
    Poll(PollCapabilitySubject),
    Receipt(ReceiptCapabilitySubject),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UncheckedSearchLoopCapabilityV1 {
    schema_id: String,
    schema_version: String,
    capability_id: ProtocolId,
    token_digest: Digest,
    command_id: SearchLoopCapabilityCommand,
    resident_identity_ref: ProtocolId,
    loop_id: ProtocolId,
    state_binding: SearchLoopCapabilityStateBinding,
    context_binding_digest: Digest,
    subject: SearchLoopCapabilitySubject,
    replay_policy: SearchLoopCapabilityReplayPolicy,
    status: SearchLoopCapabilityStatus,
    issued_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    consumed_at_revision: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    consumption_receipt_ref: Option<ProtocolId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    revocation_reason_ref: Option<ProtocolId>,
}

impl UncheckedSearchLoopCapabilityV1 {
    pub fn capability_id(&self) -> &ProtocolId {
        &self.capability_id
    }

    pub fn token_digest(&self) -> &Digest {
        &self.token_digest
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchLoopCapabilityV1(UncheckedSearchLoopCapabilityV1);

impl SearchLoopCapabilityV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn issue(
        capability_id: ProtocolId,
        token_digest: Digest,
        resident_identity_ref: ProtocolId,
        loop_id: ProtocolId,
        state_binding: SearchLoopCapabilityStateBinding,
        context_binding_digest: Digest,
        subject: SearchLoopCapabilitySubject,
        issued_at_unix_ms: u64,
        expires_at_unix_ms: u64,
    ) -> Result<Self, SearchLoopCapabilityValidationError> {
        let (command_id, replay_policy) = match &subject {
            SearchLoopCapabilitySubject::Advance(_) => (
                SearchLoopCapabilityCommand::Advance,
                SearchLoopCapabilityReplayPolicy::OneShot,
            ),
            SearchLoopCapabilitySubject::Poll(_) => (
                SearchLoopCapabilityCommand::Poll,
                SearchLoopCapabilityReplayPolicy::StateBoundIdempotent,
            ),
            SearchLoopCapabilitySubject::Receipt(_) => (
                SearchLoopCapabilityCommand::Receipt,
                SearchLoopCapabilityReplayPolicy::TerminalIdempotent,
            ),
        };
        Self::from_unchecked(UncheckedSearchLoopCapabilityV1 {
            schema_id: SEARCH_LOOP_CAPABILITY_SCHEMA_ID.to_owned(),
            schema_version: SEARCH_LOOP_CAPABILITY_SCHEMA_VERSION.to_owned(),
            capability_id,
            token_digest,
            command_id,
            resident_identity_ref,
            loop_id,
            state_binding,
            context_binding_digest,
            subject,
            replay_policy,
            status: SearchLoopCapabilityStatus::Active,
            issued_at_unix_ms,
            expires_at_unix_ms,
            consumed_at_revision: None,
            consumption_receipt_ref: None,
            revocation_reason_ref: None,
        })
    }

    pub fn from_unchecked(
        capability: UncheckedSearchLoopCapabilityV1,
    ) -> Result<Self, SearchLoopCapabilityValidationError> {
        validate_capability(&capability)?;
        Ok(Self(capability))
    }

    pub fn wire(&self) -> &UncheckedSearchLoopCapabilityV1 {
        &self.0
    }

    pub fn capability_id(&self) -> &ProtocolId {
        &self.0.capability_id
    }

    pub fn token_digest(&self) -> &Digest {
        &self.0.token_digest
    }

    pub fn command(&self) -> SearchLoopCapabilityCommand {
        self.0.command_id
    }

    pub fn resident_identity_ref(&self) -> &ProtocolId {
        &self.0.resident_identity_ref
    }

    pub fn loop_id(&self) -> &ProtocolId {
        &self.0.loop_id
    }

    pub fn state_binding(&self) -> &SearchLoopCapabilityStateBinding {
        &self.0.state_binding
    }

    pub fn context_binding_digest(&self) -> &Digest {
        &self.0.context_binding_digest
    }

    pub fn subject(&self) -> &SearchLoopCapabilitySubject {
        &self.0.subject
    }

    pub fn replay_policy(&self) -> SearchLoopCapabilityReplayPolicy {
        self.0.replay_policy
    }

    pub fn status(&self) -> SearchLoopCapabilityStatus {
        self.0.status
    }

    pub fn issued_at_unix_ms(&self) -> u64 {
        self.0.issued_at_unix_ms
    }

    pub fn expires_at_unix_ms(&self) -> u64 {
        self.0.expires_at_unix_ms
    }

    pub fn consumed_at_revision(&self) -> Option<u64> {
        self.0.consumed_at_revision
    }

    pub fn consumption_receipt_ref(&self) -> Option<&ProtocolId> {
        self.0.consumption_receipt_ref.as_ref()
    }

    pub fn revocation_reason_ref(&self) -> Option<&ProtocolId> {
        self.0.revocation_reason_ref.as_ref()
    }

    pub fn authorize(
        &self,
        command: SearchLoopCapabilityCommand,
        resident_identity_ref: &ProtocolId,
        loop_id: &ProtocolId,
        state: &ValidatedContextProductStateV1,
        observed_at_unix_ms: u64,
    ) -> Result<(), SearchLoopCapabilityValidationError> {
        if self.status() != SearchLoopCapabilityStatus::Active {
            return Err(SearchLoopCapabilityValidationError::Status);
        }
        if observed_at_unix_ms < self.issued_at_unix_ms() {
            return Err(SearchLoopCapabilityValidationError::NotYetValid);
        }
        if observed_at_unix_ms >= self.expires_at_unix_ms() {
            return Err(SearchLoopCapabilityValidationError::Expired);
        }
        if self.command() != command
            || self.resident_identity_ref() != resident_identity_ref
            || self.loop_id() != loop_id
        {
            return Err(SearchLoopCapabilityValidationError::Binding);
        }
        let binding = self.state_binding();
        if binding.run_id() != state.run_id()
            || binding.revision() != state.revision()
            || binding.state_digest() != state.state_digest()
            || binding.authority_receipt_ref() != &state.wire().authority_receipt_ref
            || self.context_binding_digest() != state.context_binding_digest()
        {
            return Err(SearchLoopCapabilityValidationError::Binding);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchLoopCapabilityMutation {
    Issue(Box<SearchLoopCapabilityV1>),
    Consume(SearchLoopCapabilityConsumption),
    Revoke(SearchLoopCapabilityRevocation),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchLoopCapabilitySpend {
    capability_id: ProtocolId,
    token_digest: Digest,
}

impl SearchLoopCapabilitySpend {
    pub fn new(capability_id: ProtocolId, token_digest: Digest) -> Self {
        Self {
            capability_id,
            token_digest,
        }
    }

    pub fn capability_id(&self) -> &ProtocolId {
        &self.capability_id
    }

    pub fn token_digest(&self) -> &Digest {
        &self.token_digest
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchLoopCapabilityConsumption {
    capability_id: ProtocolId,
    token_digest: Digest,
    consumed_at_revision: u64,
    consumption_receipt_ref: ProtocolId,
}

impl SearchLoopCapabilityConsumption {
    pub fn new(
        capability_id: ProtocolId,
        token_digest: Digest,
        consumed_at_revision: u64,
        consumption_receipt_ref: ProtocolId,
    ) -> Self {
        Self {
            capability_id,
            token_digest,
            consumed_at_revision,
            consumption_receipt_ref,
        }
    }

    pub fn capability_id(&self) -> &ProtocolId {
        &self.capability_id
    }

    pub fn token_digest(&self) -> &Digest {
        &self.token_digest
    }

    pub fn consumed_at_revision(&self) -> u64 {
        self.consumed_at_revision
    }

    pub fn consumption_receipt_ref(&self) -> &ProtocolId {
        &self.consumption_receipt_ref
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchLoopCapabilityRevocation {
    capability_id: ProtocolId,
    token_digest: Digest,
    reason_ref: ProtocolId,
}

impl SearchLoopCapabilityRevocation {
    pub fn new(capability_id: ProtocolId, token_digest: Digest, reason_ref: ProtocolId) -> Self {
        Self {
            capability_id,
            token_digest,
            reason_ref,
        }
    }

    pub fn capability_id(&self) -> &ProtocolId {
        &self.capability_id
    }

    pub fn token_digest(&self) -> &Digest {
        &self.token_digest
    }

    pub fn reason_ref(&self) -> &ProtocolId {
        &self.reason_ref
    }
}

pub fn apply_capability_mutations(
    capabilities: &mut Vec<UncheckedSearchLoopCapabilityV1>,
    mutations: &[SearchLoopCapabilityMutation],
) -> Result<(), SearchLoopCapabilityValidationError> {
    for mutation in mutations {
        match mutation {
            SearchLoopCapabilityMutation::Issue(capability) => {
                if capabilities.iter().any(|existing| {
                    existing.capability_id == *capability.capability_id()
                        || existing.token_digest == *capability.token_digest()
                }) {
                    return Err(SearchLoopCapabilityValidationError::DuplicateIdentity);
                }
                capabilities.push(capability.wire().clone());
            }
            SearchLoopCapabilityMutation::Consume(consumption) => {
                let capability = capabilities
                    .iter_mut()
                    .find(|capability| {
                        capability.capability_id == *consumption.capability_id()
                            && capability.token_digest == *consumption.token_digest()
                    })
                    .ok_or(SearchLoopCapabilityValidationError::MissingIdentity)?;
                if capability.command_id != SearchLoopCapabilityCommand::Advance
                    || capability.status != SearchLoopCapabilityStatus::Active
                {
                    return Err(SearchLoopCapabilityValidationError::Status);
                }
                capability.status = SearchLoopCapabilityStatus::Consumed;
                capability.consumed_at_revision = Some(consumption.consumed_at_revision());
                capability.consumption_receipt_ref =
                    Some(consumption.consumption_receipt_ref().clone());
            }
            SearchLoopCapabilityMutation::Revoke(revocation) => {
                let capability = capabilities
                    .iter_mut()
                    .find(|capability| {
                        capability.capability_id == *revocation.capability_id()
                            && capability.token_digest == *revocation.token_digest()
                    })
                    .ok_or(SearchLoopCapabilityValidationError::MissingIdentity)?;
                if capability.status != SearchLoopCapabilityStatus::Active {
                    return Err(SearchLoopCapabilityValidationError::Status);
                }
                capability.status = SearchLoopCapabilityStatus::Revoked;
                capability.revocation_reason_ref = Some(revocation.reason_ref().clone());
            }
        }
    }
    capabilities.sort_by(|left, right| {
        left.token_digest
            .cmp(&right.token_digest)
            .then_with(|| left.capability_id.cmp(&right.capability_id))
    });
    for capability in capabilities.iter() {
        validate_capability(capability)?;
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchLoopCapabilityValidationError {
    SchemaIdentity,
    CommandSubject,
    ReplayPolicy,
    Status,
    Expiry,
    DuplicateIdentity,
    MissingIdentity,
    Binding,
    NotYetValid,
    Expired,
    InvalidToken,
}

impl fmt::Display for SearchLoopCapabilityValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::SchemaIdentity => "invalid search-loop capability schema identity",
            Self::CommandSubject => "search-loop capability command and subject do not match",
            Self::ReplayPolicy => "search-loop capability replay policy does not match command",
            Self::Status => "invalid search-loop capability status transition fields",
            Self::Expiry => "search-loop capability expiry must be after issuance",
            Self::DuplicateIdentity => "duplicate search-loop capability identity",
            Self::MissingIdentity => "search-loop capability identity is missing",
            Self::Binding => "search-loop capability binding does not match authoritative state",
            Self::NotYetValid => "search-loop capability is not valid yet",
            Self::Expired => "search-loop capability has expired",
            Self::InvalidToken => "invalid opaque search-loop capability token",
        })
    }
}

impl std::error::Error for SearchLoopCapabilityValidationError {}

fn validate_capability(
    capability: &UncheckedSearchLoopCapabilityV1,
) -> Result<(), SearchLoopCapabilityValidationError> {
    if capability.schema_id != SEARCH_LOOP_CAPABILITY_SCHEMA_ID
        || capability.schema_version != SEARCH_LOOP_CAPABILITY_SCHEMA_VERSION
    {
        return Err(SearchLoopCapabilityValidationError::SchemaIdentity);
    }
    let expected_replay_policy = match (&capability.command_id, &capability.subject) {
        (SearchLoopCapabilityCommand::Advance, SearchLoopCapabilitySubject::Advance(_)) => {
            SearchLoopCapabilityReplayPolicy::OneShot
        }
        (SearchLoopCapabilityCommand::Poll, SearchLoopCapabilitySubject::Poll(_)) => {
            SearchLoopCapabilityReplayPolicy::StateBoundIdempotent
        }
        (SearchLoopCapabilityCommand::Receipt, SearchLoopCapabilitySubject::Receipt(_)) => {
            SearchLoopCapabilityReplayPolicy::TerminalIdempotent
        }
        _ => return Err(SearchLoopCapabilityValidationError::CommandSubject),
    };
    if capability.replay_policy != expected_replay_policy {
        return Err(SearchLoopCapabilityValidationError::ReplayPolicy);
    }
    let status_fields_valid = match capability.status {
        SearchLoopCapabilityStatus::Active => {
            capability.consumed_at_revision.is_none()
                && capability.consumption_receipt_ref.is_none()
                && capability.revocation_reason_ref.is_none()
        }
        SearchLoopCapabilityStatus::Consumed => {
            capability.command_id == SearchLoopCapabilityCommand::Advance
                && capability.consumed_at_revision.is_some()
                && capability.consumption_receipt_ref.is_some()
                && capability.revocation_reason_ref.is_none()
        }
        SearchLoopCapabilityStatus::Revoked => {
            capability.consumed_at_revision.is_none()
                && capability.consumption_receipt_ref.is_none()
                && capability.revocation_reason_ref.is_some()
        }
    };
    if !status_fields_valid {
        return Err(SearchLoopCapabilityValidationError::Status);
    }
    if capability.expires_at_unix_ms <= capability.issued_at_unix_ms {
        return Err(SearchLoopCapabilityValidationError::Expiry);
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/search_capability.rs"]
mod tests;

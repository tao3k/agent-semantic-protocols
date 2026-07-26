use serde::{Deserialize, Serialize};

use crate::primitives::{Digest, ProtocolId};

macro_rules! execution_protocol_id {
    ($name:ident) => {
        #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
        #[serde(transparent)]
        struct $name(ProtocolId);

        impl From<ProtocolId> for $name {
            fn from(value: ProtocolId) -> Self {
                Self(value)
            }
        }

        impl $name {
            fn as_protocol_id(&self) -> &ProtocolId {
                &self.0
            }
        }
    };
}

macro_rules! execution_digest {
    ($name:ident) => {
        #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
        #[serde(transparent)]
        struct $name(Digest);

        impl From<Digest> for $name {
            fn from(value: Digest) -> Self {
                Self(value)
            }
        }

        impl $name {
            fn as_digest(&self) -> &Digest {
                &self.0
            }
        }
    };
}

macro_rules! execution_counter {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
        #[serde(transparent)]
        struct $name(u64);

        impl From<u64> for $name {
            fn from(value: u64) -> Self {
                Self(value)
            }
        }

        impl $name {
            fn get(self) -> u64 {
                self.0
            }
        }
    };
}

execution_protocol_id!(ExecutionAdmissionId);
execution_protocol_id!(ExecutionGrantId);
execution_protocol_id!(ExecutionAttemptId);
execution_protocol_id!(ExecutionGroupId);
execution_protocol_id!(ExecutionProviderId);
execution_protocol_id!(ExecutionOperationId);
execution_protocol_id!(ExecutionBudgetReservationId);
execution_protocol_id!(ExecutionProviderIdempotencyKey);
execution_protocol_id!(ExecutionResultReceiptRef);
execution_protocol_id!(ExecutionRevocationReasonCode);

execution_digest!(ExecutionAdmissionDigest);
execution_digest!(ExecutionActionKey);
execution_digest!(ExecutionProgramDigest);
execution_digest!(ExecutionGrantDigest);
execution_digest!(ExecutionAttemptDigest);
execution_digest!(ExecutionResolvedInputDigest);
execution_digest!(ExecutionPolicyDigest);
execution_digest!(ExecutionDependencyDigest);
execution_digest!(ExecutionBudgetChargeKey);
execution_digest!(ExecutionResultDigest);

impl ExecutionProgramDigest {
    fn as_digest_mut(&mut self) -> &mut Digest {
        &mut self.0
    }
}

execution_counter!(ExecutionLeaseFence);
execution_counter!(ExecutionExpiresAtMs);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
struct ExecutionStageSet(Vec<ProtocolId>);

impl From<Vec<ProtocolId>> for ExecutionStageSet {
    fn from(value: Vec<ProtocolId>) -> Self {
        Self(value)
    }
}

impl ExecutionStageSet {
    fn as_slice(&self) -> &[ProtocolId] {
        &self.0
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdmittedExecutionAuthority {
    admission_id: ExecutionAdmissionId,
    admission_digest: ExecutionAdmissionDigest,
    action_key: ExecutionActionKey,
    program_digest: ExecutionProgramDigest,
    execution_group_id: ExecutionGroupId,
    stage_ids: ExecutionStageSet,
    provider_id: ExecutionProviderId,
    operation: ExecutionOperationId,
    resolved_input_digest: ExecutionResolvedInputDigest,
    policy_digest: ExecutionPolicyDigest,
    dependency_digest: ExecutionDependencyDigest,
    budget_reservation_id: ExecutionBudgetReservationId,
    budget_charge_key: ExecutionBudgetChargeKey,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GrantedExecutionAuthority {
    admission_id: ExecutionAdmissionId,
    admission_digest: ExecutionAdmissionDigest,
    grant_id: ExecutionGrantId,
    grant_digest: ExecutionGrantDigest,
    action_key: ExecutionActionKey,
    program_digest: ExecutionProgramDigest,
    execution_group_id: ExecutionGroupId,
    stage_ids: ExecutionStageSet,
    provider_id: ExecutionProviderId,
    operation: ExecutionOperationId,
    resolved_input_digest: ExecutionResolvedInputDigest,
    policy_digest: ExecutionPolicyDigest,
    dependency_digest: ExecutionDependencyDigest,
    budget_reservation_id: ExecutionBudgetReservationId,
    budget_charge_key: ExecutionBudgetChargeKey,
    lease_fence: ExecutionLeaseFence,
    expires_at_ms: ExecutionExpiresAtMs,
    effect_class: super::EffectClass,
    provider_idempotency_key: ExecutionProviderIdempotencyKey,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InFlightExecutionAuthority {
    admission_id: ExecutionAdmissionId,
    admission_digest: ExecutionAdmissionDigest,
    grant_id: ExecutionGrantId,
    grant_digest: ExecutionGrantDigest,
    action_key: ExecutionActionKey,
    attempt_id: ExecutionAttemptId,
    attempt_digest: ExecutionAttemptDigest,
    provider_id: ExecutionProviderId,
    operation: ExecutionOperationId,
    program_digest: ExecutionProgramDigest,
    execution_group_id: ExecutionGroupId,
    stage_ids: ExecutionStageSet,
    resolved_input_digest: ExecutionResolvedInputDigest,
    policy_digest: ExecutionPolicyDigest,
    dependency_digest: ExecutionDependencyDigest,
    budget_reservation_id: ExecutionBudgetReservationId,
    budget_charge_key: ExecutionBudgetChargeKey,
    effect_class: super::EffectClass,
    lease_fence: ExecutionLeaseFence,
    expires_at_ms: ExecutionExpiresAtMs,
    provider_idempotency_key: ExecutionProviderIdempotencyKey,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConsumedExecutionAuthority {
    admission_id: ExecutionAdmissionId,
    grant_id: ExecutionGrantId,
    grant_digest: ExecutionGrantDigest,
    action_key: ExecutionActionKey,
    attempt_id: ExecutionAttemptId,
    program_digest: ExecutionProgramDigest,
    execution_group_id: ExecutionGroupId,
    stage_ids: ExecutionStageSet,
    provider_id: ExecutionProviderId,
    operation: ExecutionOperationId,
    result_receipt_ref: ExecutionResultReceiptRef,
    result_digest: ExecutionResultDigest,
    lease_fence: ExecutionLeaseFence,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RevokedExecutionAuthority {
    reason_code: ExecutionRevocationReasonCode,
    program_digest: ExecutionProgramDigest,
    execution_group_id: ExecutionGroupId,
    stage_ids: ExecutionStageSet,
    provider_id: ExecutionProviderId,
    operation: ExecutionOperationId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    admission_id: Option<ExecutionAdmissionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    grant_id: Option<ExecutionGrantId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    action_key: Option<ExecutionActionKey>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ExecutionAuthority {
    Admitted(AdmittedExecutionAuthority),
    Granted(GrantedExecutionAuthority),
    InFlight(InFlightExecutionAuthority),
    Consumed(ConsumedExecutionAuthority),
    Revoked(RevokedExecutionAuthority),
}

impl ExecutionAuthority {
    pub fn admitted(
        event: &super::ActionAdmitted,
        program_digest: Digest,
        provider_id: ProtocolId,
        operation: ProtocolId,
    ) -> Self {
        Self::Admitted(AdmittedExecutionAuthority {
            admission_id: event.admission_id.clone().into(),
            admission_digest: event.admission_digest.clone().into(),
            action_key: event.action_key.clone().into(),
            program_digest: program_digest.into(),
            execution_group_id: event.execution_group_id.clone().into(),
            stage_ids: event.stage_ids.clone().into(),
            provider_id: provider_id.into(),
            operation: operation.into(),
            resolved_input_digest: event.resolved_input_digest.clone().into(),
            policy_digest: event.policy_digest.clone().into(),
            dependency_digest: event.dependency_digest.clone().into(),
            budget_reservation_id: event.budget_reservation_id.clone().into(),
            budget_charge_key: event.budget_charge_key.clone().into(),
        })
    }

    pub fn granted(
        admitted: &Self,
        event: &super::ExecutionGrantIssued,
        provider_idempotency_key: ProtocolId,
    ) -> Option<Self> {
        let Self::Admitted(admitted) = admitted else {
            return None;
        };
        Some(Self::Granted(GrantedExecutionAuthority {
            admission_id: admitted.admission_id.clone(),
            admission_digest: admitted.admission_digest.clone(),
            grant_id: event.grant_id.clone().into(),
            grant_digest: event.grant_digest.clone().into(),
            action_key: admitted.action_key.clone(),
            program_digest: admitted.program_digest.clone(),
            execution_group_id: admitted.execution_group_id.clone(),
            stage_ids: admitted.stage_ids.clone(),
            provider_id: admitted.provider_id.clone(),
            operation: admitted.operation.clone(),
            resolved_input_digest: admitted.resolved_input_digest.clone(),
            policy_digest: admitted.policy_digest.clone(),
            dependency_digest: admitted.dependency_digest.clone(),
            budget_reservation_id: admitted.budget_reservation_id.clone(),
            budget_charge_key: admitted.budget_charge_key.clone(),
            lease_fence: event.lease_fence.into(),
            expires_at_ms: event.expires_at_ms.into(),
            effect_class: event.effect_class,
            provider_idempotency_key: provider_idempotency_key.into(),
        }))
    }

    pub fn in_flight(granted: &Self, event: &super::ExecutionStarted) -> Option<Self> {
        let Self::Granted(granted) = granted else {
            return None;
        };
        Some(Self::InFlight(InFlightExecutionAuthority {
            admission_id: granted.admission_id.clone(),
            admission_digest: granted.admission_digest.clone(),
            grant_id: granted.grant_id.clone(),
            grant_digest: granted.grant_digest.clone(),
            action_key: granted.action_key.clone(),
            attempt_id: event.attempt_id.clone().into(),
            attempt_digest: event.attempt_digest.clone().into(),
            provider_id: granted.provider_id.clone(),
            operation: granted.operation.clone(),
            program_digest: granted.program_digest.clone(),
            execution_group_id: granted.execution_group_id.clone(),
            stage_ids: granted.stage_ids.clone(),
            resolved_input_digest: granted.resolved_input_digest.clone(),
            policy_digest: granted.policy_digest.clone(),
            dependency_digest: granted.dependency_digest.clone(),
            budget_reservation_id: granted.budget_reservation_id.clone(),
            budget_charge_key: granted.budget_charge_key.clone(),
            effect_class: granted.effect_class,
            lease_fence: granted.lease_fence,
            expires_at_ms: granted.expires_at_ms,
            provider_idempotency_key: granted.provider_idempotency_key.clone(),
        }))
    }

    pub fn consumed(in_flight: &Self, event: &super::ExecutionConsumed) -> Option<Self> {
        let Self::InFlight(in_flight) = in_flight else {
            return None;
        };
        Some(Self::Consumed(ConsumedExecutionAuthority {
            admission_id: in_flight.admission_id.clone(),
            grant_id: in_flight.grant_id.clone(),
            grant_digest: in_flight.grant_digest.clone(),
            action_key: in_flight.action_key.clone(),
            attempt_id: in_flight.attempt_id.clone(),
            program_digest: in_flight.program_digest.clone(),
            execution_group_id: in_flight.execution_group_id.clone(),
            stage_ids: in_flight.stage_ids.clone(),
            provider_id: in_flight.provider_id.clone(),
            operation: in_flight.operation.clone(),
            result_receipt_ref: event.result_receipt_ref.clone().into(),
            result_digest: event.result_digest.clone().into(),
            lease_fence: in_flight.lease_fence,
        }))
    }

    pub fn revoked(authority: &Self, event: &super::ExecutionRevoked) -> Self {
        Self::Revoked(RevokedExecutionAuthority {
            reason_code: event.reason_code.clone().into(),
            program_digest: authority.program_digest().clone().into(),
            execution_group_id: authority.execution_group_id().clone().into(),
            stage_ids: authority.stage_ids().to_vec().into(),
            provider_id: authority.provider_id().clone().into(),
            operation: authority.operation().clone().into(),
            admission_id: authority.admission_id().cloned().map(Into::into),
            grant_id: authority.grant_id().cloned().map(Into::into),
            action_key: authority.action_key().cloned().map(Into::into),
        })
    }

    pub fn execution_group_id(&self) -> &ProtocolId {
        match self {
            Self::Admitted(authority) => authority.execution_group_id.as_protocol_id(),
            Self::Granted(authority) => authority.execution_group_id.as_protocol_id(),
            Self::InFlight(authority) => authority.execution_group_id.as_protocol_id(),
            Self::Consumed(authority) => authority.execution_group_id.as_protocol_id(),
            Self::Revoked(authority) => authority.execution_group_id.as_protocol_id(),
        }
    }

    pub fn stage_ids(&self) -> &[ProtocolId] {
        match self {
            Self::Admitted(authority) => authority.stage_ids.as_slice(),
            Self::Granted(authority) => authority.stage_ids.as_slice(),
            Self::InFlight(authority) => authority.stage_ids.as_slice(),
            Self::Consumed(authority) => authority.stage_ids.as_slice(),
            Self::Revoked(authority) => authority.stage_ids.as_slice(),
        }
    }

    pub fn program_digest(&self) -> &Digest {
        match self {
            Self::Admitted(authority) => authority.program_digest.as_digest(),
            Self::Granted(authority) => authority.program_digest.as_digest(),
            Self::InFlight(authority) => authority.program_digest.as_digest(),
            Self::Consumed(authority) => authority.program_digest.as_digest(),
            Self::Revoked(authority) => authority.program_digest.as_digest(),
        }
    }

    pub fn program_digest_mut(&mut self) -> &mut Digest {
        match self {
            Self::Admitted(authority) => authority.program_digest.as_digest_mut(),
            Self::Granted(authority) => authority.program_digest.as_digest_mut(),
            Self::InFlight(authority) => authority.program_digest.as_digest_mut(),
            Self::Consumed(authority) => authority.program_digest.as_digest_mut(),
            Self::Revoked(authority) => authority.program_digest.as_digest_mut(),
        }
    }

    pub fn admission_id(&self) -> Option<&ProtocolId> {
        match self {
            Self::Admitted(authority) => Some(authority.admission_id.as_protocol_id()),
            Self::Granted(authority) => Some(authority.admission_id.as_protocol_id()),
            Self::InFlight(authority) => Some(authority.admission_id.as_protocol_id()),
            Self::Consumed(authority) => Some(authority.admission_id.as_protocol_id()),
            Self::Revoked(authority) => authority
                .admission_id
                .as_ref()
                .map(ExecutionAdmissionId::as_protocol_id),
        }
    }

    pub fn grant_id(&self) -> Option<&ProtocolId> {
        match self {
            Self::Granted(authority) => Some(authority.grant_id.as_protocol_id()),
            Self::InFlight(authority) => Some(authority.grant_id.as_protocol_id()),
            Self::Consumed(authority) => Some(authority.grant_id.as_protocol_id()),
            Self::Revoked(authority) => authority
                .grant_id
                .as_ref()
                .map(ExecutionGrantId::as_protocol_id),
            Self::Admitted(_) => None,
        }
    }

    pub fn grant_digest(&self) -> Option<&Digest> {
        match self {
            Self::Granted(authority) => Some(authority.grant_digest.as_digest()),
            Self::InFlight(authority) => Some(authority.grant_digest.as_digest()),
            Self::Consumed(authority) => Some(authority.grant_digest.as_digest()),
            Self::Admitted(_) | Self::Revoked(_) => None,
        }
    }

    pub fn admission_digest(&self) -> Option<&Digest> {
        match self {
            Self::Admitted(authority) => Some(authority.admission_digest.as_digest()),
            Self::Granted(authority) => Some(authority.admission_digest.as_digest()),
            Self::InFlight(authority) => Some(authority.admission_digest.as_digest()),
            Self::Consumed(_) | Self::Revoked(_) => None,
        }
    }

    pub fn action_key(&self) -> Option<&Digest> {
        match self {
            Self::Admitted(authority) => Some(authority.action_key.as_digest()),
            Self::Granted(authority) => Some(authority.action_key.as_digest()),
            Self::InFlight(authority) => Some(authority.action_key.as_digest()),
            Self::Consumed(authority) => Some(authority.action_key.as_digest()),
            Self::Revoked(authority) => authority
                .action_key
                .as_ref()
                .map(ExecutionActionKey::as_digest),
        }
    }

    pub fn provider_id(&self) -> &ProtocolId {
        match self {
            Self::Admitted(authority) => authority.provider_id.as_protocol_id(),
            Self::Granted(authority) => authority.provider_id.as_protocol_id(),
            Self::InFlight(authority) => authority.provider_id.as_protocol_id(),
            Self::Consumed(authority) => authority.provider_id.as_protocol_id(),
            Self::Revoked(authority) => authority.provider_id.as_protocol_id(),
        }
    }

    pub fn operation(&self) -> &ProtocolId {
        match self {
            Self::Admitted(authority) => authority.operation.as_protocol_id(),
            Self::Granted(authority) => authority.operation.as_protocol_id(),
            Self::InFlight(authority) => authority.operation.as_protocol_id(),
            Self::Consumed(authority) => authority.operation.as_protocol_id(),
            Self::Revoked(authority) => authority.operation.as_protocol_id(),
        }
    }

    pub fn resolved_input_digest(&self) -> Option<&Digest> {
        match self {
            Self::Admitted(authority) => Some(authority.resolved_input_digest.as_digest()),
            Self::Granted(authority) => Some(authority.resolved_input_digest.as_digest()),
            Self::InFlight(authority) => Some(authority.resolved_input_digest.as_digest()),
            Self::Consumed(_) | Self::Revoked(_) => None,
        }
    }

    pub fn policy_digest(&self) -> Option<&Digest> {
        match self {
            Self::Admitted(authority) => Some(authority.policy_digest.as_digest()),
            Self::Granted(authority) => Some(authority.policy_digest.as_digest()),
            Self::InFlight(authority) => Some(authority.policy_digest.as_digest()),
            Self::Consumed(_) | Self::Revoked(_) => None,
        }
    }

    pub fn dependency_digest(&self) -> Option<&Digest> {
        match self {
            Self::Admitted(authority) => Some(authority.dependency_digest.as_digest()),
            Self::Granted(authority) => Some(authority.dependency_digest.as_digest()),
            Self::InFlight(authority) => Some(authority.dependency_digest.as_digest()),
            Self::Consumed(_) | Self::Revoked(_) => None,
        }
    }

    pub fn budget_reservation_id(&self) -> Option<&ProtocolId> {
        match self {
            Self::Admitted(authority) => Some(authority.budget_reservation_id.as_protocol_id()),
            Self::Granted(authority) => Some(authority.budget_reservation_id.as_protocol_id()),
            Self::InFlight(authority) => Some(authority.budget_reservation_id.as_protocol_id()),
            Self::Consumed(_) | Self::Revoked(_) => None,
        }
    }

    pub fn budget_charge_key(&self) -> Option<&Digest> {
        match self {
            Self::Admitted(authority) => Some(authority.budget_charge_key.as_digest()),
            Self::Granted(authority) => Some(authority.budget_charge_key.as_digest()),
            Self::InFlight(authority) => Some(authority.budget_charge_key.as_digest()),
            Self::Consumed(_) | Self::Revoked(_) => None,
        }
    }

    pub fn attempt_id(&self) -> Option<&ProtocolId> {
        match self {
            Self::InFlight(authority) => Some(authority.attempt_id.as_protocol_id()),
            Self::Consumed(authority) => Some(authority.attempt_id.as_protocol_id()),
            Self::Admitted(_) | Self::Granted(_) | Self::Revoked(_) => None,
        }
    }

    pub fn attempt_digest(&self) -> Option<&Digest> {
        match self {
            Self::InFlight(authority) => Some(authority.attempt_digest.as_digest()),
            Self::Admitted(_) | Self::Granted(_) | Self::Consumed(_) | Self::Revoked(_) => None,
        }
    }

    pub fn lease_fence(&self) -> Option<u64> {
        match self {
            Self::Granted(authority) => Some(authority.lease_fence.get()),
            Self::InFlight(authority) => Some(authority.lease_fence.get()),
            Self::Consumed(authority) => Some(authority.lease_fence.get()),
            Self::Admitted(_) | Self::Revoked(_) => None,
        }
    }

    pub fn expires_at_ms(&self) -> Option<u64> {
        match self {
            Self::Granted(authority) => Some(authority.expires_at_ms.get()),
            Self::InFlight(authority) => Some(authority.expires_at_ms.get()),
            Self::Admitted(_) | Self::Consumed(_) | Self::Revoked(_) => None,
        }
    }

    pub fn effect_class(&self) -> Option<super::EffectClass> {
        match self {
            Self::Granted(authority) => Some(authority.effect_class),
            Self::InFlight(authority) => Some(authority.effect_class),
            Self::Admitted(_) | Self::Consumed(_) | Self::Revoked(_) => None,
        }
    }

    pub fn provider_idempotency_key(&self) -> Option<&ProtocolId> {
        match self {
            Self::Granted(authority) => Some(authority.provider_idempotency_key.as_protocol_id()),
            Self::InFlight(authority) => Some(authority.provider_idempotency_key.as_protocol_id()),
            Self::Admitted(_) | Self::Consumed(_) | Self::Revoked(_) => None,
        }
    }

    pub fn result_receipt_ref(&self) -> Option<&ProtocolId> {
        match self {
            Self::Consumed(authority) => Some(authority.result_receipt_ref.as_protocol_id()),
            Self::Admitted(_) | Self::Granted(_) | Self::InFlight(_) | Self::Revoked(_) => None,
        }
    }

    pub fn result_digest(&self) -> Option<&Digest> {
        match self {
            Self::Consumed(authority) => Some(authority.result_digest.as_digest()),
            Self::Admitted(_) | Self::Granted(_) | Self::InFlight(_) | Self::Revoked(_) => None,
        }
    }

    pub fn reason_code(&self) -> Option<&ProtocolId> {
        match self {
            Self::Revoked(authority) => Some(authority.reason_code.as_protocol_id()),
            Self::Admitted(_) | Self::Granted(_) | Self::InFlight(_) | Self::Consumed(_) => None,
        }
    }
}

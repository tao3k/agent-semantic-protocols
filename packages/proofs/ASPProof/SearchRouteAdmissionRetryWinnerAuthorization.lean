-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteAdmissionRetryPublicationDigest

namespace ASPProof.SearchRouteAdmissionRetryWinnerAuthorization

open ASPProof.SearchRouteAdmissionRetryPublicationKey
open ASPProof.SearchRouteAdmissionRetryPublicationLedger

universe u

/--
The grant is an authoritative statement, not a copy of untrusted request
fields. Grant activity is modeled separately so expiry and revocation cannot be
confused with scope equality.
-/
structure RecoveryGrant where
  workspaceIdentity : Nat
  ledgerGeneration : Nat
  contractVersion : Nat
  tenantScope : Nat
  retryIdentity : Nat
deriving DecidableEq, Repr

structure RecoveryRequest where
  presentedKey : RetryPublicationKey
  grant : RecoveryGrant
deriving DecidableEq, Repr

def GrantMatchesKey
    (grant : RecoveryGrant)
    (key : RetryPublicationKey) : Prop :=
  grant.workspaceIdentity = key.workspaceIdentity
    ∧ grant.ledgerGeneration = key.ledgerGeneration
    ∧ grant.contractVersion = key.contractVersion
    ∧ grant.tenantScope = key.tenantScope
    ∧ grant.retryIdentity = key.retryIdentity

instance grantMatchesKeyDecidable
    (grant : RecoveryGrant)
    (key : RetryPublicationKey) :
    Decidable (GrantMatchesKey grant key) := by
  unfold GrantMatchesKey
  infer_instance

def Authorized
    (grantActive : RecoveryGrant → Prop)
    (request : RecoveryRequest)
    (targetKey : RetryPublicationKey) : Prop :=
  grantActive request.grant
    ∧ request.presentedKey = targetKey
    ∧ GrantMatchesKey request.grant targetKey

instance authorizedDecidable
    (grantActive : RecoveryGrant → Prop)
    [DecidablePred grantActive]
    (request : RecoveryRequest)
    (targetKey : RetryPublicationKey) :
    Decidable (Authorized grantActive request targetKey) := by
  unfold Authorized
  infer_instance

inductive RecoveryOutcome (Certificate : Type u) where
  | released (winner : Certificate)
  | missing
  | denied
deriving DecidableEq, Repr

/--
Authorization is decided before the winner ledger is observed. Unauthorized
requests receive `denied` independently of whether a winner exists.
-/
def authorizedRecover
    {Certificate : Type u}
    (grantActive : RecoveryGrant → Prop)
    [DecidablePred grantActive]
    (ledger : PublicationLedger RetryPublicationKey Certificate)
    (request : RecoveryRequest)
    (targetKey : RetryPublicationKey) :
    RecoveryOutcome Certificate :=
  if Authorized grantActive request targetKey then
    match ledger targetKey with
    | some winner => RecoveryOutcome.released winner
    | none => RecoveryOutcome.missing
  else
    RecoveryOutcome.denied

theorem authorization_preserves_key_and_all_scope_dimensions
    {grantActive : RecoveryGrant → Prop}
    {request : RecoveryRequest}
    {targetKey : RetryPublicationKey}
    (authorized : Authorized grantActive request targetKey) :
    grantActive request.grant
      ∧ request.presentedKey = targetKey
      ∧ request.grant.workspaceIdentity = targetKey.workspaceIdentity
      ∧ request.grant.ledgerGeneration = targetKey.ledgerGeneration
      ∧ request.grant.contractVersion = targetKey.contractVersion
      ∧ request.grant.tenantScope = targetKey.tenantScope
      ∧ request.grant.retryIdentity = targetKey.retryIdentity := by
  rcases authorized with
    ⟨active, presented, workspace, generation, contract, tenant, retry⟩
  exact
    ⟨active,
      presented,
      workspace,
      generation,
      contract,
      tenant,
      retry⟩

theorem unauthorized_request_is_denied_before_winner_lookup
    {Certificate : Type u}
    (grantActive : RecoveryGrant → Prop)
    [DecidablePred grantActive]
    (ledger : PublicationLedger RetryPublicationKey Certificate)
    (request : RecoveryRequest)
    (targetKey : RetryPublicationKey)
    (unauthorized : ¬ Authorized grantActive request targetKey) :
    authorizedRecover grantActive ledger request targetKey =
      RecoveryOutcome.denied := by
  simp [authorizedRecover, unauthorized]

theorem authorized_request_recovers_existing_winner
    {Certificate : Type u}
    (grantActive : RecoveryGrant → Prop)
    [DecidablePred grantActive]
    (ledger : PublicationLedger RetryPublicationKey Certificate)
    (request : RecoveryRequest)
    (targetKey : RetryPublicationKey)
    (winner : Certificate)
    (authorized : Authorized grantActive request targetKey)
    (existing : ledger targetKey = some winner) :
    authorizedRecover grantActive ledger request targetKey =
      RecoveryOutcome.released winner := by
  simp [authorizedRecover, authorized, existing]

def grantForBaseKey : RecoveryGrant :=
  { workspaceIdentity := baseKey.workspaceIdentity
    ledgerGeneration := baseKey.ledgerGeneration
    contractVersion := baseKey.contractVersion
    tenantScope := baseKey.tenantScope
    retryIdentity := baseKey.retryIdentity }

def exactKeyRequest : RecoveryRequest :=
  { presentedKey := baseKey
    grant := grantForBaseKey }

def inactiveGrant (_grant : RecoveryGrant) : Prop :=
  False

def activeGrant (_grant : RecoveryGrant) : Prop :=
  True

instance inactiveGrantDecidable : DecidablePred inactiveGrant :=
  fun _grant => isFalse (by simp [inactiveGrant])

instance activeGrantDecidable : DecidablePred activeGrant :=
  fun _grant => isTrue (by simp [activeGrant])

def wrongTenantGrant : RecoveryGrant :=
  { grantForBaseKey with tenantScope := 1 }

def wrongTenantRequest : RecoveryRequest :=
  { presentedKey := baseKey
    grant := wrongTenantGrant }

theorem known_key_and_matching_scope_without_active_grant_is_not_authorized :
    exactKeyRequest.presentedKey = baseKey
      ∧ GrantMatchesKey exactKeyRequest.grant baseKey
      ∧ ¬ Authorized inactiveGrant exactKeyRequest baseKey := by
  decide

theorem active_grant_with_wrong_tenant_scope_is_not_authorized :
    activeGrant wrongTenantRequest.grant
      ∧ wrongTenantRequest.presentedKey = baseKey
      ∧ ¬ GrantMatchesKey wrongTenantRequest.grant baseKey
      ∧ ¬ Authorized activeGrant wrongTenantRequest baseKey := by
  decide

def exampleWinnerLedger :
    PublicationLedger RetryPublicationKey Bool :=
  fun key => if key = baseKey then some true else none

def unsafeLookupBeforeAuthorization
    {Certificate : Type u}
    (ledger : PublicationLedger RetryPublicationKey Certificate)
    (request : RecoveryRequest) :
    Option Certificate :=
  ledger request.presentedKey

theorem lookup_before_authorization_leaks_winner_to_wrong_tenant :
    unsafeLookupBeforeAuthorization
        exampleWinnerLedger
        wrongTenantRequest =
      some true
      ∧ ¬ Authorized activeGrant wrongTenantRequest baseKey := by
  decide

end ASPProof.SearchRouteAdmissionRetryWinnerAuthorization

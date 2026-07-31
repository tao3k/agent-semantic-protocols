import ASPProof.SearchRouteAdmissionRetryWinnerAuthorization

namespace ASPProof.SearchRouteAdmissionRetryAuthorizationStamp

open ASPProof.SearchRouteAdmissionRetryPublicationKey
open ASPProof.SearchRouteAdmissionRetryWinnerAuthorization

universe u

structure AuthorizationSnapshot where
  generation : Nat
  grantActive : RecoveryGrant → Bool

structure AuthorizationDecision where
  authorizationGeneration : Nat
  grant : RecoveryGrant
  targetKey : RetryPublicationKey
  authorizedAtDecision : Bool
deriving DecidableEq, Repr

def SafeToRelease
    (snapshot : AuthorizationSnapshot)
    (decision : AuthorizationDecision) : Prop :=
  decision.authorizedAtDecision = true
    ∧ decision.authorizationGeneration = snapshot.generation
    ∧ snapshot.grantActive decision.grant = true

instance safeToReleaseDecidable
    (snapshot : AuthorizationSnapshot)
    (decision : AuthorizationDecision) :
    Decidable (SafeToRelease snapshot decision) := by
  unfold SafeToRelease
  infer_instance

def releaseWithAuthorizationRecheck
    {Winner : Type u}
    (snapshot : AuthorizationSnapshot)
    (decision : AuthorizationDecision)
    (winner : Winner) :
    RecoveryOutcome Winner :=
  if SafeToRelease snapshot decision then
    RecoveryOutcome.released winner
  else
    RecoveryOutcome.denied

theorem safe_release_preserves_decision_generation_and_current_activity
    {snapshot : AuthorizationSnapshot}
    {decision : AuthorizationDecision}
    (safe : SafeToRelease snapshot decision) :
    decision.authorizedAtDecision = true
      ∧ decision.authorizationGeneration = snapshot.generation
      ∧ snapshot.grantActive decision.grant = true :=
  safe

theorem failed_freshness_recheck_denies_release
    {Winner : Type u}
    (snapshot : AuthorizationSnapshot)
    (decision : AuthorizationDecision)
    (winner : Winner)
    (notFresh : ¬ SafeToRelease snapshot decision) :
    releaseWithAuthorizationRecheck snapshot decision winner =
      RecoveryOutcome.denied := by
  simp [releaseWithAuthorizationRecheck, notFresh]

def originalAuthorizationSnapshot : AuthorizationSnapshot :=
  { generation := 0
    grantActive := fun _grant => true }

def revokedAuthorizationSnapshot : AuthorizationSnapshot :=
  { generation := 0
    grantActive := fun _grant => false }

def nextGenerationAuthorizationSnapshot : AuthorizationSnapshot :=
  { generation := 1
    grantActive := fun _grant => true }

def originalAuthorizationDecision : AuthorizationDecision :=
  { authorizationGeneration := 0
    grant := grantForBaseKey
    targetKey := baseKey
    authorizedAtDecision := true }

theorem original_active_generation_allows_release :
    SafeToRelease
        originalAuthorizationSnapshot
        originalAuthorizationDecision
      ∧ releaseWithAuthorizationRecheck
          originalAuthorizationSnapshot
          originalAuthorizationDecision
          true =
        RecoveryOutcome.released true := by
  decide

theorem same_generation_revocation_invalidates_prior_decision :
    originalAuthorizationDecision.authorizedAtDecision = true
      ∧ ¬ SafeToRelease
        revokedAuthorizationSnapshot
        originalAuthorizationDecision
      ∧ releaseWithAuthorizationRecheck
          revokedAuthorizationSnapshot
          originalAuthorizationDecision
          true =
        RecoveryOutcome.denied := by
  decide

theorem generation_advance_invalidates_reactivated_grant_decision :
    nextGenerationAuthorizationSnapshot.grantActive
          originalAuthorizationDecision.grant =
        true
      ∧ originalAuthorizationDecision.authorizationGeneration <
        nextGenerationAuthorizationSnapshot.generation
      ∧ ¬ SafeToRelease
        nextGenerationAuthorizationSnapshot
        originalAuthorizationDecision
      ∧ releaseWithAuthorizationRecheck
          nextGenerationAuthorizationSnapshot
          originalAuthorizationDecision
          true =
        RecoveryOutcome.denied := by
  decide

def unsafeReleaseFromDecisionBit
    {Winner : Type u}
    (decision : AuthorizationDecision)
    (winner : Winner) :
    RecoveryOutcome Winner :=
  if decision.authorizedAtDecision then
    RecoveryOutcome.released winner
  else
    RecoveryOutcome.denied

theorem decision_bit_without_freshness_recheck_releases_stale_winner :
    unsafeReleaseFromDecisionBit originalAuthorizationDecision true =
        RecoveryOutcome.released true
      ∧ ¬ SafeToRelease
        nextGenerationAuthorizationSnapshot
        originalAuthorizationDecision := by
  decide

end ASPProof.SearchRouteAdmissionRetryAuthorizationStamp

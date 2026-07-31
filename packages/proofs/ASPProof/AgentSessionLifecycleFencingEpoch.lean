import ASPProof.AgentSessionLifecycleConcurrentLinearizability

namespace ASPProof.AgentSessionLifecycleFencingEpoch

open AgentSessionTransactionalGenerationLifecycle
open AgentSessionLifecycleCrashRecovery

structure AuthorityToken where
  slot : ResidentSlot
  generation : Nat
  fencingEpoch : Nat
  revision : Nat
  stateDigest : Nat
  capabilityDigest : Nat
  controllerId : Nat
  expiresAt : Nat
  deriving DecidableEq

structure CurrentAuthority where
  slot : ResidentSlot
  generation : Nat
  fencingEpoch : Nat
  revision : Nat
  stateDigest : Nat
  capabilityDigest : Nat
  now : Nat
  deriving DecidableEq

def TokenCurrent (current : CurrentAuthority) (token : AuthorityToken) : Prop :=
  token.slot = current.slot ∧
  token.generation = current.generation ∧
  token.fencingEpoch = current.fencingEpoch ∧
  token.revision = current.revision ∧
  token.stateDigest = current.stateDigest ∧
  token.capabilityDigest = current.capabilityDigest ∧
  current.now < token.expiresAt

inductive LifecycleEffect where
  | retire
  | releasePath
  | bindGeneration
  | dispatch
  | commitTerminal
  deriving DecidableEq

structure CapabilitySet where
  retire : Bool
  releasePath : Bool
  bindGeneration : Bool
  dispatch : Bool
  commitTerminal : Bool
  deriving DecidableEq

def Allows (capabilities : CapabilitySet) : LifecycleEffect → Prop
  | .retire => capabilities.retire = true
  | .releasePath => capabilities.releasePath = true
  | .bindGeneration => capabilities.bindGeneration = true
  | .dispatch => capabilities.dispatch = true
  | .commitTerminal => capabilities.commitTerminal = true

def Authorized
    (current : CurrentAuthority)
    (token : AuthorityToken)
    (capabilities : CapabilitySet)
    (effect : LifecycleEffect) : Prop :=
  TokenCurrent current token ∧ Allows capabilities effect

def FenceAdvance (old new : CurrentAuthority) : Prop :=
  old.slot = new.slot ∧ old.fencingEpoch < new.fencingEpoch

structure TerminalIngestion where
  dispatch : DispatchRecord
  receipt : DispatchCompletionReceipt
  validCompletion : ValidDispatchCompletion dispatch receipt

theorem current_token_binds_slot
    {current : CurrentAuthority} {token : AuthorityToken}
    (valid : TokenCurrent current token) : token.slot = current.slot :=
  valid.1

theorem current_token_binds_generation
    {current : CurrentAuthority} {token : AuthorityToken}
    (valid : TokenCurrent current token) : token.generation = current.generation :=
  valid.2.1

theorem current_token_binds_epoch
    {current : CurrentAuthority} {token : AuthorityToken}
    (valid : TokenCurrent current token) : token.fencingEpoch = current.fencingEpoch :=
  valid.2.2.1

theorem current_token_binds_revision
    {current : CurrentAuthority} {token : AuthorityToken}
    (valid : TokenCurrent current token) : token.revision = current.revision :=
  valid.2.2.2.1

theorem current_token_binds_state_digest
    {current : CurrentAuthority} {token : AuthorityToken}
    (valid : TokenCurrent current token) : token.stateDigest = current.stateDigest :=
  valid.2.2.2.2.1

theorem current_token_binds_capability_digest
    {current : CurrentAuthority} {token : AuthorityToken}
    (valid : TokenCurrent current token) :
    token.capabilityDigest = current.capabilityDigest :=
  valid.2.2.2.2.2.1

theorem current_token_is_unexpired
    {current : CurrentAuthority} {token : AuthorityToken}
    (valid : TokenCurrent current token) : current.now < token.expiresAt :=
  valid.2.2.2.2.2.2

theorem stale_generation_rejects_token
    {current : CurrentAuthority} {token : AuthorityToken}
    (stale : token.generation ≠ current.generation) :
    ¬ TokenCurrent current token := by
  intro valid
  exact stale valid.2.1

theorem stale_epoch_rejects_token
    {current : CurrentAuthority} {token : AuthorityToken}
    (stale : token.fencingEpoch ≠ current.fencingEpoch) :
    ¬ TokenCurrent current token := by
  intro valid
  exact stale valid.2.2.1

theorem stale_revision_rejects_token
    {current : CurrentAuthority} {token : AuthorityToken}
    (stale : token.revision ≠ current.revision) :
    ¬ TokenCurrent current token := by
  intro valid
  exact stale valid.2.2.2.1

theorem state_drift_rejects_token
    {current : CurrentAuthority} {token : AuthorityToken}
    (drift : token.stateDigest ≠ current.stateDigest) :
    ¬ TokenCurrent current token := by
  intro valid
  exact drift valid.2.2.2.2.1

theorem capability_drift_rejects_token
    {current : CurrentAuthority} {token : AuthorityToken}
    (drift : token.capabilityDigest ≠ current.capabilityDigest) :
    ¬ TokenCurrent current token := by
  intro valid
  exact drift valid.2.2.2.2.2.1

theorem expired_token_is_rejected
    {current : CurrentAuthority} {token : AuthorityToken}
    (expired : token.expiresAt ≤ current.now) :
    ¬ TokenCurrent current token := by
  intro valid
  exact (Nat.not_lt_of_ge expired) valid.2.2.2.2.2.2

theorem noncurrent_token_authorizes_no_effect
    {current : CurrentAuthority} {token : AuthorityToken}
    (notCurrent : ¬ TokenCurrent current token) :
    ∀ capabilities effect, ¬ Authorized current token capabilities effect := by
  intro capabilities effect authorized
  exact notCurrent authorized.1

theorem stale_epoch_cannot_retire
    {current : CurrentAuthority} {token : AuthorityToken}
    (stale : token.fencingEpoch ≠ current.fencingEpoch) :
    ∀ capabilities, ¬ Authorized current token capabilities .retire := by
  intro capabilities
  exact noncurrent_token_authorizes_no_effect
    (stale_epoch_rejects_token stale) capabilities .retire

theorem stale_epoch_cannot_release
    {current : CurrentAuthority} {token : AuthorityToken}
    (stale : token.fencingEpoch ≠ current.fencingEpoch) :
    ∀ capabilities, ¬ Authorized current token capabilities .releasePath := by
  intro capabilities
  exact noncurrent_token_authorizes_no_effect
    (stale_epoch_rejects_token stale) capabilities .releasePath

theorem stale_epoch_cannot_bind
    {current : CurrentAuthority} {token : AuthorityToken}
    (stale : token.fencingEpoch ≠ current.fencingEpoch) :
    ∀ capabilities, ¬ Authorized current token capabilities .bindGeneration := by
  intro capabilities
  exact noncurrent_token_authorizes_no_effect
    (stale_epoch_rejects_token stale) capabilities .bindGeneration

theorem stale_epoch_cannot_dispatch
    {current : CurrentAuthority} {token : AuthorityToken}
    (stale : token.fencingEpoch ≠ current.fencingEpoch) :
    ∀ capabilities, ¬ Authorized current token capabilities .dispatch := by
  intro capabilities
  exact noncurrent_token_authorizes_no_effect
    (stale_epoch_rejects_token stale) capabilities .dispatch

theorem stale_epoch_cannot_commit_terminal
    {current : CurrentAuthority} {token : AuthorityToken}
    (stale : token.fencingEpoch ≠ current.fencingEpoch) :
    ∀ capabilities, ¬ Authorized current token capabilities .commitTerminal := by
  intro capabilities
  exact noncurrent_token_authorizes_no_effect
    (stale_epoch_rejects_token stale) capabilities .commitTerminal

theorem fence_advance_invalidates_old_epoch_token
    {old new : CurrentAuthority} {token : AuthorityToken}
    (advance : FenceAdvance old new)
    (oldEpoch : token.fencingEpoch = old.fencingEpoch) :
    ¬ TokenCurrent new token := by
  apply stale_epoch_rejects_token
  intro same
  have oldEqualsNew : old.fencingEpoch = new.fencingEpoch := oldEpoch.symm.trans same
  exact (Nat.ne_of_lt advance.2) oldEqualsNew

theorem fence_advance_has_no_self_loop
    {authority : CurrentAuthority} : ¬ FenceAdvance authority authority := by
  intro advance
  exact (Nat.lt_irrefl _ advance.2)

theorem fence_advance_has_no_two_cycle
    {left right : CurrentAuthority}
    (forward : FenceAdvance left right)
    (backward : FenceAdvance right left) : False :=
  (Nat.not_lt_of_ge (Nat.le_of_lt backward.2)) forward.2

theorem authorized_retire_requires_retire_capability
    {current : CurrentAuthority} {token : AuthorityToken}
    {capabilities : CapabilitySet}
    (authorized : Authorized current token capabilities .retire) :
    capabilities.retire = true :=
  authorized.2

theorem current_controller_can_ingest_old_generation_completion
    (ingestion : TerminalIngestion) :
    ingestion.receipt.dispatchKey = ingestion.dispatch.dispatchKey :=
  ingestion.validCompletion.2.1

end ASPProof.AgentSessionLifecycleFencingEpoch

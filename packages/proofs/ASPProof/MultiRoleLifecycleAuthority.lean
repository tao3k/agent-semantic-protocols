import ASPProof.HookSessionMaterialization

namespace ASPProof.MultiRoleLifecycleAuthority

structure InstanceKey where
  rootSessionId : Nat
  roleId : Nat
  physicalGeneration : Nat
  deriving Repr, DecidableEq

structure RoleAuthority where
  rootSessionId : Nat
  roleId : Nat
  authorityRevision : Nat
  roleGeneration : Nat
  activeRoleCount : Nat
  activeTotalCount : Nat
  maxPerRole : Nat
  maxTotal : Nat
  deriving Repr, DecidableEq

structure RoleClaim where
  rootSessionId : Nat
  roleId : Nat
  expectedAuthorityRevision : Nat
  observedRoleGeneration : Nat
  claimToken : Nat
  deriving Repr, DecidableEq

def CanClaimRole
    (authority : RoleAuthority)
    (claim : RoleClaim) : Prop :=
  claim.rootSessionId = authority.rootSessionId ∧
  claim.roleId = authority.roleId ∧
  claim.roleId ≠ 0 ∧
  claim.expectedAuthorityRevision = authority.authorityRevision ∧
  claim.observedRoleGeneration = authority.roleGeneration ∧
  authority.activeRoleCount < authority.maxPerRole ∧
  authority.activeTotalCount < authority.maxTotal ∧
  claim.claimToken ≠ 0

instance canClaimRoleDecidable
    (authority : RoleAuthority)
    (claim : RoleClaim) :
    Decidable (CanClaimRole authority claim) := by
  unfold CanClaimRole
  infer_instance

/-- Counterexample predicate representing a legacy claim gate without quotas. -/
def LegacyCanClaimWithoutQuota
    (authority : RoleAuthority)
    (claim : RoleClaim) : Prop :=
  claim.rootSessionId = authority.rootSessionId ∧
  claim.roleId = authority.roleId ∧
  claim.expectedAuthorityRevision = authority.authorityRevision ∧
  claim.observedRoleGeneration = authority.roleGeneration

instance legacyCanClaimWithoutQuotaDecidable
    (authority : RoleAuthority)
    (claim : RoleClaim) :
    Decidable (LegacyCanClaimWithoutQuota authority claim) := by
  unfold LegacyCanClaimWithoutQuota
  infer_instance

def commitRoleClaim
    (authority : RoleAuthority)
    (_claim : RoleClaim)
    (_admissible : CanClaimRole authority _claim) : RoleAuthority :=
  { authority with
    authorityRevision := authority.authorityRevision + 1
    roleGeneration := authority.roleGeneration + 1
    activeRoleCount := authority.activeRoleCount + 1
    activeTotalCount := authority.activeTotalCount + 1 }

inductive InstancePhase where
  | active
  | terminating
  | terminated
  deriving Repr, DecidableEq

structure ResidentInstance where
  key : InstanceKey
  childSessionId : Nat
  messageTargetId : Nat
  phase : InstancePhase
  deriving Repr, DecidableEq

structure DispatchLease where
  key : InstanceKey
  authorityRevision : Nat
  observedRevision : Nat
  expiresRevision : Nat
  childSessionId : Nat
  messageTargetId : Nat
  deriving Repr, DecidableEq

def CanDispatch
    (authority : RoleAuthority)
    (resident : ResidentInstance)
    (lease : DispatchLease) : Prop :=
  resident.phase = .active ∧
  resident.key.rootSessionId = authority.rootSessionId ∧
  resident.key.roleId = authority.roleId ∧
  resident.key.physicalGeneration = authority.roleGeneration ∧
  lease.key = resident.key ∧
  lease.authorityRevision = authority.authorityRevision ∧
  lease.observedRevision ≤ authority.authorityRevision ∧
  authority.authorityRevision ≤ lease.expiresRevision ∧
  lease.childSessionId = resident.childSessionId ∧
  lease.messageTargetId = resident.messageTargetId

instance canDispatchDecidable
    (authority : RoleAuthority)
    (resident : ResidentInstance)
    (lease : DispatchLease) :
    Decidable (CanDispatch authority resident lease) := by
  unfold CanDispatch
  infer_instance

structure TerminationClaim where
  key : InstanceKey
  expectedAuthorityRevision : Nat
  deriving Repr, DecidableEq

def CanBeginTermination
    (authority : RoleAuthority)
    (resident : ResidentInstance)
    (claim : TerminationClaim) : Prop :=
  resident.phase = .active ∧
  claim.key = resident.key ∧
  resident.key.rootSessionId = authority.rootSessionId ∧
  resident.key.roleId = authority.roleId ∧
  resident.key.physicalGeneration = authority.roleGeneration ∧
  claim.expectedAuthorityRevision = authority.authorityRevision

instance canBeginTerminationDecidable
    (authority : RoleAuthority)
    (resident : ResidentInstance)
    (claim : TerminationClaim) :
    Decidable (CanBeginTermination authority resident claim) := by
  unfold CanBeginTermination
  infer_instance

structure TerminationState where
  authority : RoleAuthority
  resident : ResidentInstance
  deriving Repr, DecidableEq

def beginTermination
    (authority : RoleAuthority)
    (resident : ResidentInstance)
    (claim : TerminationClaim)
    (_admissible : CanBeginTermination authority resident claim) :
    TerminationState :=
  { authority :=
      { authority with
        authorityRevision := authority.authorityRevision + 1 }
    resident := { resident with phase := .terminating } }

def CanCompleteTermination (state : TerminationState) : Prop :=
  state.resident.phase = .terminating ∧
  0 < state.authority.activeRoleCount ∧
  0 < state.authority.activeTotalCount

instance canCompleteTerminationDecidable (state : TerminationState) :
    Decidable (CanCompleteTermination state) := by
  unfold CanCompleteTermination
  infer_instance

def completeTermination
    (state : TerminationState)
    (_admissible : CanCompleteTermination state) : TerminationState :=
  { authority :=
      { state.authority with
        authorityRevision := state.authority.authorityRevision + 1
        activeRoleCount := state.authority.activeRoleCount - 1
        activeTotalCount := state.authority.activeTotalCount - 1 }
    resident := { state.resident with phase := .terminated } }

def CanReplaceTarget
    (resident : ResidentInstance)
    (newMessageTargetId : Nat) : Prop :=
  resident.phase = .terminated ∧
  newMessageTargetId ≠ 0 ∧
  newMessageTargetId ≠ resident.messageTargetId

instance canReplaceTargetDecidable
    (resident : ResidentInstance)
    (newMessageTargetId : Nat) :
    Decidable (CanReplaceTarget resident newMessageTargetId) := by
  unfold CanReplaceTarget
  infer_instance

def baseAuthority : RoleAuthority :=
  { rootSessionId := 100
    roleId := 7
    authorityRevision := 10
    roleGeneration := 1
    activeRoleCount := 1
    activeTotalCount := 2
    maxPerRole := 2
    maxTotal := 4 }

def baseClaim : RoleClaim :=
  { rootSessionId := 100
    roleId := 7
    expectedAuthorityRevision := 10
    observedRoleGeneration := 1
    claimToken := 701 }

def perRoleQuotaAuthority : RoleAuthority :=
  { baseAuthority with activeRoleCount := 2 }

def totalQuotaAuthority : RoleAuthority :=
  { baseAuthority with activeTotalCount := 4 }

def activeInstance : ResidentInstance :=
  { key :=
      { rootSessionId := 100
        roleId := 7
        physicalGeneration := 1 }
    childSessionId := 200
    messageTargetId := 200
    phase := .active }

def activeLease : DispatchLease :=
  { key := activeInstance.key
    authorityRevision := 10
    observedRevision := 10
    expiresRevision := 11
    childSessionId := 200
    messageTargetId := 200 }

def terminationClaim : TerminationClaim :=
  { key := activeInstance.key
    expectedAuthorityRevision := 10 }

def terminatingState : TerminationState :=
  beginTermination baseAuthority activeInstance terminationClaim (by decide)

def terminatedState : TerminationState :=
  completeTermination terminatingState (by decide)

theorem generic_role_claim_is_admissible :
    CanClaimRole baseAuthority baseClaim := by
  decide

theorem per_role_quota_blocks_claim :
    ¬ CanClaimRole perRoleQuotaAuthority baseClaim := by
  decide

theorem total_quota_blocks_claim :
    ¬ CanClaimRole totalQuotaAuthority baseClaim := by
  decide

theorem legacy_claim_gate_ignores_exhausted_quota :
    LegacyCanClaimWithoutQuota perRoleQuotaAuthority baseClaim := by
  decide

theorem distinct_roles_produce_distinct_instance_keys
    (left right : InstanceKey)
    (different : left.roleId ≠ right.roleId) :
    left ≠ right := by
  intro equal
  exact different (congrArg InstanceKey.roleId equal)

theorem claim_commit_increments_generation_revision_and_counts
    (admissible : CanClaimRole authority claim) :
    let committed := commitRoleClaim authority claim admissible
    committed.authorityRevision = authority.authorityRevision + 1 ∧
    committed.roleGeneration = authority.roleGeneration + 1 ∧
    committed.activeRoleCount = authority.activeRoleCount + 1 ∧
    committed.activeTotalCount = authority.activeTotalCount + 1 := by
  exact ⟨rfl, rfl, rfl, rfl⟩

theorem dispatch_is_admissible_before_termination :
    CanDispatch baseAuthority activeInstance activeLease := by
  decide

theorem termination_is_admissible_at_same_snapshot :
    CanBeginTermination baseAuthority activeInstance terminationClaim := by
  decide

theorem termination_linearization_invalidates_old_dispatch :
    ¬ CanDispatch
      terminatingState.authority
      terminatingState.resident
      activeLease := by
  decide

theorem completed_termination_releases_quota :
    terminatedState.authority.activeRoleCount = 0 ∧
    terminatedState.authority.activeTotalCount = 1 := by
  exact ⟨rfl, rfl⟩

theorem replacement_cannot_reuse_old_message_target :
    ¬ CanReplaceTarget terminatedState.resident 200 := by
  decide

theorem replacement_with_fresh_target_is_admissible :
    CanReplaceTarget terminatedState.resident 300 := by
  decide

inductive RecoveryPhase where
  | configuredAbsent
  | claimed
  | spawned
  | bound
  | active
  deriving Repr, DecidableEq

def requiredGraphHops : RecoveryPhase → Nat
  | .configuredAbsent => 5
  | .claimed => 4
  | .spawned => 3
  | .bound => 2
  | .active => 1

structure RecoveryCost where
  graphHops : Nat
  interactionRounds : Nat
  exposedTokens : Nat
  deriving Repr, DecidableEq

def SafeRecoveryPath
    (phase : RecoveryPhase)
    (cost : RecoveryCost) : Prop :=
  requiredGraphHops phase ≤ cost.graphHops

instance safeRecoveryPathDecidable
    (phase : RecoveryPhase)
    (cost : RecoveryCost) :
    Decidable (SafeRecoveryPath phase cost) := by
  unfold SafeRecoveryPath
  infer_instance

def spawnedFrontierCost : RecoveryCost :=
  { graphHops := 3
    interactionRounds := 1
    exposedTokens := 240 }

def restartLoopCost : RecoveryCost :=
  { graphHops := 7
    interactionRounds := 4
    exposedTokens := 1100 }

theorem spawned_frontier_path_is_safe :
    SafeRecoveryPath .spawned spawnedFrontierCost := by
  decide

theorem spawned_frontier_path_is_graph_shortest
    (safe : SafeRecoveryPath .spawned candidate) :
    spawnedFrontierCost.graphHops ≤ candidate.graphHops := by
  exact safe

theorem spawned_frontier_reduces_rounds_and_tokens :
    spawnedFrontierCost.interactionRounds <
        restartLoopCost.interactionRounds ∧
      spawnedFrontierCost.exposedTokens <
        restartLoopCost.exposedTokens := by
  decide

end ASPProof.MultiRoleLifecycleAuthority

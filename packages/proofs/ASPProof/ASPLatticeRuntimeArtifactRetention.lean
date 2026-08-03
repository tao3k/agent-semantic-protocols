import Std

namespace ASPProof.ASPLatticeRuntimeArtifactRetention

abbrev ArtifactId := Nat
abbrev ContentDigest := Nat

structure Candidate where
  artifact : ArtifactId
  digest : ContentDigest
  verified : Bool
deriving DecidableEq

structure ProfileState where
  current : ArtifactId
  candidate : Option Candidate := none
  retiredPendingCleanup : Bool := false
deriving DecidableEq

def storedExecutableCount (state : ProfileState) : Nat :=
  1 + (if state.candidate.isSome then 1 else 0) +
    (if state.retiredPendingCleanup then 1 else 0)

def refreshAdmitted (state : ProfileState) : Prop :=
  state.candidate.isNone ∧ ¬state.retiredPendingCleanup

def WellFormed (state : ProfileState) : Prop :=
  state.candidate.isNone ∨ ¬state.retiredPendingCleanup

def selectExecutable (state : ProfileState) : ArtifactId :=
  state.current

def publish (_state : ProfileState) (candidate : Candidate) : Option ProfileState :=
  if candidate.verified then
    some {
      current := candidate.artifact
      candidate := none
      retiredPendingCleanup := false
    }
  else
    none

theorem quiescent_stores_exactly_one (current : ArtifactId) :
    storedExecutableCount {
      current := current
      candidate := none
      retiredPendingCleanup := false
    } = 1 := by
  rfl

theorem ordinary_refresh_stores_exactly_two
    (current : ArtifactId) (candidate : Candidate) :
    storedExecutableCount {
      current := current
      candidate := some candidate
      retiredPendingCleanup := false
    } = 2 := by
  simp [storedExecutableCount]

theorem retired_cleanup_blocks_refresh (current : ArtifactId) :
    ¬refreshAdmitted {
      current := current
      candidate := none
      retiredPendingCleanup := true
    } := by
  simp [refreshAdmitted]

theorem well_formed_storage_is_bounded (state : ProfileState)
    (wellFormed : WellFormed state) :
    storedExecutableCount state ≤ 2 := by
  cases candidate : state.candidate <;>
    cases retired : state.retiredPendingCleanup <;>
    simp [WellFormed, storedExecutableCount, candidate, retired] at wellFormed ⊢

theorem unverified_candidate_cannot_publish
    (state : ProfileState) (candidate : Candidate)
    (unverified : candidate.verified = false) :
    publish state candidate = none := by
  simp [publish, unverified]

theorem verified_publication_returns_to_one
    (state : ProfileState) (candidate : Candidate)
    (verified : candidate.verified = true) :
    ∃ published,
      publish state candidate = some published ∧
      published.current = candidate.artifact ∧
      storedExecutableCount published = 1 := by
  refine ⟨{
    current := candidate.artifact
    candidate := none
    retiredPendingCleanup := false
  }, ?_⟩
  simp [publish, verified, storedExecutableCount]

theorem selection_never_uses_candidate_or_retired (state : ProfileState) :
    selectExecutable state = state.current := by
  rfl

structure AppendOnlyDigestStore where
  artifacts : List (ContentDigest × ArtifactId)
deriving DecidableEq

def appendOnlyRefresh
    (store : AppendOnlyDigestStore)
    (digest : ContentDigest)
    (artifact : ArtifactId) : AppendOnlyDigestStore :=
  { artifacts := (digest, artifact) :: store.artifacts }

theorem append_only_refresh_strictly_grows
    (store : AppendOnlyDigestStore)
    (digest : ContentDigest)
    (artifact : ArtifactId) :
    (appendOnlyRefresh store digest artifact).artifacts.length =
      store.artifacts.length + 1 := by
  simp [appendOnlyRefresh]

theorem three_refreshes_violate_two_artifact_bound
    (firstDigest secondDigest thirdDigest : ContentDigest)
    (first second third : ArtifactId) :
    let initial : AppendOnlyDigestStore := { artifacts := [] }
    let afterFirst := appendOnlyRefresh initial firstDigest first
    let afterSecond := appendOnlyRefresh afterFirst secondDigest second
    let afterThird := appendOnlyRefresh afterSecond thirdDigest third
    afterThird.artifacts.length > 2 := by
  simp [appendOnlyRefresh]

def storedWithRollbackHistory (rollbackGenerations : Nat) : Nat :=
  1 + rollbackGenerations

theorem current_plus_two_rollbacks_violates_lattice_bound :
    storedWithRollbackHistory 2 = 3 ∧
    storedWithRollbackHistory 2 > 2 := by
  decide

end ASPProof.ASPLatticeRuntimeArtifactRetention

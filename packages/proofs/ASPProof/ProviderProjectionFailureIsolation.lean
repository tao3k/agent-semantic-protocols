namespace ASPProof.ProviderProjectionFailureIsolation

inductive ProjectionState where
  | ready
  | syntaxUnavailable
  deriving DecidableEq, Repr

structure OwnerProjection where
  ownerPath : String
  sourceLeafDigest : String
  state : ProjectionState
  diagnostic : Option String
  items : List String
  relations : List String
  deriving DecidableEq, Repr

def OwnerProjection.admissible (owner : OwnerProjection) : Prop :=
  match owner.state with
  | .ready => owner.diagnostic = none
  | .syntaxUnavailable =>
      owner.items = [] ∧ owner.relations = [] ∧
        ∃ message, owner.diagnostic = some message ∧ message ≠ ""

structure GenerationCandidate where
  frameIdentityValid : Bool
  sourceMembershipComplete : Bool
  owners : List OwnerProjection
  deriving DecidableEq, Repr

def Publishable (candidate : GenerationCandidate) : Prop :=
  candidate.frameIdentityValid = true ∧
    candidate.sourceMembershipComplete = true ∧
    ∀ owner ∈ candidate.owners, owner.admissible

def LegacyPublishable (candidate : GenerationCandidate) : Prop :=
  Publishable candidate ∧
    ∀ owner ∈ candidate.owners, owner.state = .ready

def unavailableOwner : OwnerProjection :=
  { ownerPath := "src/unavailable.ss"
    sourceLeafDigest := "blake3-256:leaf"
    state := .syntaxUnavailable
    diagnostic := some "source-syntax-unavailable"
    items := []
    relations := [] }

def isolatedCandidate : GenerationCandidate :=
  { frameIdentityValid := true
    sourceMembershipComplete := true
    owners := [unavailableOwner] }

theorem syntax_unavailable_owner_is_admissible :
    unavailableOwner.admissible := by
  simp [OwnerProjection.admissible, unavailableOwner]

theorem syntax_failure_does_not_remove_source_membership :
    Publishable isolatedCandidate := by
  simp [Publishable, isolatedCandidate, syntax_unavailable_owner_is_admissible]

theorem legacy_all_or_nothing_rejects_isolated_candidate :
    ¬ LegacyPublishable isolatedCandidate := by
  simp [LegacyPublishable, isolatedCandidate, unavailableOwner]

theorem unavailable_owner_cannot_publish_items
    (owner : OwnerProjection)
    (unavailable : owner.state = .syntaxUnavailable)
    (admitted : owner.admissible) :
    owner.items = [] ∧ owner.relations = [] := by
  simp [OwnerProjection.admissible, unavailable] at admitted
  exact ⟨admitted.1, admitted.2.1⟩

theorem invalid_frame_cannot_publish
    (candidate : GenerationCandidate)
    (invalid : candidate.frameIdentityValid = false) :
    ¬ Publishable candidate := by
  intro published
  simp [Publishable, invalid] at published

theorem incomplete_source_membership_cannot_publish
    (candidate : GenerationCandidate)
    (incomplete : candidate.sourceMembershipComplete = false) :
    ¬ Publishable candidate := by
  intro published
  simp [Publishable, incomplete] at published

end ASPProof.ProviderProjectionFailureIsolation

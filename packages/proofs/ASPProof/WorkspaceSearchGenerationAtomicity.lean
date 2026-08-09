import Std

namespace ASPProof.WorkspaceSearchGenerationAtomicity

inductive SegmentKind
  | source
  | exact
  | search
  deriving DecidableEq, Repr

structure State where
  segments : Nat → String → SegmentKind → Prop
  pointer : Option (Nat × String)

def Complete (state : State) (epoch : Nat) (rootDigest : String) : Prop :=
  ∀ kind, state.segments epoch rootDigest kind

structure PreparedGeneration (state : State) where
  epoch : Nat
  rootDigest : String
  complete : Complete state epoch rootDigest

def commit (state : State) (prepared : PreparedGeneration state) : State :=
  { state with pointer := some (prepared.epoch, prepared.rootDigest) }

theorem commit_preserves_segments
    (state : State)
    (prepared : PreparedGeneration state)
    (epoch : Nat)
    (rootDigest : String)
    (kind : SegmentKind) :
    (commit state prepared).segments epoch rootDigest kind ↔
      state.segments epoch rootDigest kind := by
  rfl

theorem committed_pointer_has_complete_generation
    (state : State)
    (prepared : PreparedGeneration state) :
    Complete (commit state prepared) prepared.epoch prepared.rootDigest := by
  intro kind
  exact prepared.complete kind

structure ReadReceipt (state : State) where
  epoch : Nat
  rootDigest : String
  pointerCurrent : state.pointer = some (epoch, rootDigest)
  segmentKind : SegmentKind
  sectionPublished : state.segments epoch rootDigest segmentKind

theorem two_reads_from_one_pointer_cannot_cross_epochs
    (state : State)
    (left right : ReadReceipt state) :
    left.epoch = right.epoch ∧ left.rootDigest = right.rootDigest := by
  have same : (left.epoch, left.rootDigest) = (right.epoch, right.rootDigest) := by
    rw [← Option.some_inj]
    rw [← left.pointerCurrent, ← right.pointerCurrent]
  exact ⟨congrArg Prod.fst same, congrArg Prod.snd same⟩

inductive QueryCapability
  | mappedRead
  | socketConnect
  | databaseWrite
  | providerSpawn
  deriving DecidableEq, Repr

def AdmittedExactQueryCapability (capability : QueryCapability) : Prop :=
  capability = .mappedRead

theorem admitted_exact_query_excludes_control_plane_effects
    {capability : QueryCapability}
    (admitted : AdmittedExactQueryCapability capability) :
    capability ≠ .socketConnect ∧
      capability ≠ .databaseWrite ∧
      capability ≠ .providerSpawn := by
  subst capability
  decide

end ASPProof.WorkspaceSearchGenerationAtomicity

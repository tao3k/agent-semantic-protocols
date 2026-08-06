namespace ASPProof.RuntimeGenerationMemoryAuthority

inductive Surface where
  | search
  | exactQuery
  | lifecycleSupervisor
  | explicitMutation
  deriving DecidableEq, Repr

inductive GenerationEffect where
  | readPublished
  | returnGenerationRequired
  | startBuilder
  | enterAtomicCommit
  | publishTerminal
  | decodeCompleteGeneration
  deriving DecidableEq, Repr

def permits (surface : Surface) (effect : GenerationEffect) : Prop :=
  match surface, effect with
  | .search, .readPublished => True
  | .search, .returnGenerationRequired => True
  | .exactQuery, .readPublished => True
  | .exactQuery, .returnGenerationRequired => True
  | .lifecycleSupervisor, _ => True
  | .explicitMutation, _ => True
  | _, _ => False

theorem search_cannot_start_builder :
    ¬ permits .search .startBuilder := by
  simp [permits]

theorem exact_query_cannot_start_builder :
    ¬ permits .exactQuery .startBuilder := by
  simp [permits]

theorem short_lived_search_cannot_decode_complete_generation :
    ¬ permits .search .decodeCompleteGeneration := by
  simp [permits]

theorem query_deadline_cannot_transfer_writer_authority
    (surface : Surface)
    (readOnly : surface = .search ∨ surface = .exactQuery) :
    ¬ permits surface .enterAtomicCommit := by
  rcases readOnly with rfl | rfl <;> simp [permits]

structure GenerationCardinality where
  sourceBytes : Nat
  selectorFactBytes : Nat
  relationFactBytes : Nat

def linearEncodedWork (cardinality : GenerationCardinality) : Nat :=
  cardinality.sourceBytes + cardinality.selectorFactBytes + cardinality.relationFactBytes

def itemProofBytes (itemFactBytes : Nat) (_unrelatedRelationBytes : Nat) : Nat :=
  itemFactBytes

theorem item_proof_is_relation_independent
    (itemFactBytes relationBytes extraRelationBytes : Nat) :
    itemProofBytes itemFactBytes relationBytes =
      itemProofBytes itemFactBytes (relationBytes + extraRelationBytes) := by
  rfl

theorem linear_work_is_additive (cardinality : GenerationCardinality) :
    linearEncodedWork cardinality =
      cardinality.sourceBytes + cardinality.selectorFactBytes + cardinality.relationFactBytes := by
  rfl

inductive WriterPhase where
  | idle
  | building
  | committing
  | terminal
  deriving DecidableEq, Repr

def beginBuild : WriterPhase → Option WriterPhase
  | .idle => some .building
  | _ => none

def beginCommit : WriterPhase → Option WriterPhase
  | .building => some .committing
  | _ => none

def publish : WriterPhase → Option WriterPhase
  | .committing => some .terminal
  | _ => none

theorem one_builder_lease (phase : WriterPhase)
    (started : beginBuild phase = some .building) :
    beginBuild .building = none := by
  cases phase <;> simp [beginBuild] at started ⊢

theorem terminal_requires_atomic_commit (phase : WriterPhase)
    (published : publish phase = some .terminal) :
    phase = .committing := by
  cases phase <;> simp [publish] at published ⊢

structure CapturedEnvelope where
  baseIdentity : Nat
  overlayIdentity : Nat
  providerIdentity : Nat

def candidateIdentity (capture : CapturedEnvelope) : Nat :=
  capture.baseIdentity + capture.overlayIdentity + capture.providerIdentity

structure Publication where
  captured : CapturedEnvelope
  candidate : Nat

def capturedPublication (capture : CapturedEnvelope) : Publication :=
  { captured := capture, candidate := candidateIdentity capture }

theorem publication_candidate_is_capture_derived (capture : CapturedEnvelope) :
    (capturedPublication capture).candidate = candidateIdentity capture := by
  rfl

def laterMutationDoesNotRewritePublished
    (published : Publication) (_laterOverlay : Nat) : Nat :=
  published.candidate

theorem later_mutation_targets_next_epoch
    (capture : CapturedEnvelope) (laterOverlay : Nat) :
    laterMutationDoesNotRewritePublished (capturedPublication capture) laterOverlay =
      candidateIdentity capture := by
  rfl

structure CompactAuthorityCardinality where
  identityBytes : Nat
  projectResolutionBytes : Nat

def compactAuthorityBytes (cardinality : CompactAuthorityCardinality) : Nat :=
  cardinality.identityBytes + cardinality.projectResolutionBytes

theorem compact_authority_is_workspace_path_map_independent
    (cardinality : CompactAuthorityCardinality)
    (_workspacePathMapBytes : Nat) :
    compactAuthorityBytes cardinality =
      cardinality.identityBytes + cardinality.projectResolutionBytes := by
  rfl

end ASPProof.RuntimeGenerationMemoryAuthority

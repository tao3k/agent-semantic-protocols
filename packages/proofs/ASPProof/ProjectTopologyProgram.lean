-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import Std

/-!
Executable design model for RFC 10.06.18.

The finite identifiers are abstract values, not production digests.  This file
models authority separation and extension safety; it does not claim that the
current Rust implementation realizes the model.
-/

namespace ASPProof.ProjectTopologyProgram

inductive StandardProfile where
  | workspace
  | engineering
  | domain
  | file
  | structure
  | reference
  deriving DecidableEq, Repr

structure ProjectProfile where
  namespaceId : Nat
  localId : Nat
  deriving DecidableEq, Repr

inductive ProfileId where
  | standard : StandardProfile → ProfileId
  | project : ProjectProfile → ProfileId
  deriving DecidableEq, Repr

def standardProfiles : List ProfileId :=
  [ .standard .workspace
  , .standard .engineering
  , .standard .domain
  , .standard .file
  , .standard .structure
  , .standard .reference
  ]

def profileWellFormed : ProfileId → Bool
  | .standard _ => true
  | .project profile => profile.namespaceId != 0

def runtimeAuthorityProfile : ProfileId :=
  .project ⟨7, 1⟩

theorem standard_library_has_six_profiles : standardProfiles.length = 6 := by
  decide

theorem project_profile_is_admissible_without_a_kernel_variant :
    profileWellFormed runtimeAuthorityProfile = true ∧
      runtimeAuthorityProfile ∉ standardProfiles := by
  decide

def composedProfiles (left right : List ProfileId) (profile : ProfileId) : Prop :=
  profile ∈ left ∨ profile ∈ right

theorem profile_composition_is_membership_commutative
    (left right : List ProfileId) (profile : ProfileId) :
    composedProfiles left right profile ↔ composedProfiles right left profile := by
  simp only [composedProfiles]
  exact or_comm

inductive Modality where
  | parserFact
  | declared
  | derived
  | proposed
  | contested
  | accepted
  deriving DecidableEq, Repr

structure RelationFact where
  key : Nat
  value : Nat
  modality : Modality
  witnessId : Nat
  deriving DecidableEq, Repr

def projectProgramMayProduce : Modality → Bool
  | .derived | .proposed | .contested => true
  | .parserFact | .declared | .accepted => false

def factualPremise : Modality → Bool
  | .parserFact | .declared | .derived | .accepted => true
  | .proposed | .contested => false

inductive TopologyRecordOwnership where
  | sourceSegment : Nat → TopologyRecordOwnership
  | topologyGeneration : Nat → TopologyRecordOwnership
  deriving DecidableEq, Repr

def ownershipValid
    (modality : Modality)
    (ownership : TopologyRecordOwnership) : Bool :=
  match modality, ownership with
  | .parserFact, .sourceSegment _ => true
  | .declared, .sourceSegment _ => true
  | .derived, .topologyGeneration _ => true
  | .proposed, .topologyGeneration _ => true
  | .contested, .topologyGeneration _ => true
  | .accepted, .topologyGeneration _ => true
  | _, _ => false

theorem parser_fact_requires_source_segment_ownership :
    ownershipValid .parserFact (.sourceSegment 1) = true ∧
      ownershipValid .parserFact (.topologyGeneration 2) = false := by
  decide

theorem derived_fact_cannot_impersonate_source_segment_evidence :
    ownershipValid .derived (.topologyGeneration 2) = true ∧
      ownershipValid .derived (.sourceSegment 1) = false := by
  decide

theorem proposed_model_semantics_are_not_factual :
    factualPremise .proposed = false := by
  decide

theorem project_program_cannot_mint_parser_or_accepted_authority :
    projectProgramMayProduce .parserFact = false ∧
      projectProgramMayProduce .accepted = false := by
  decide

def agreesWithCore (core candidate : List RelationFact) : Bool :=
  candidate.all fun proposed =>
    core.all fun existing =>
      if proposed.key == existing.key then
        proposed.value == existing.value && proposed.modality == existing.modality
      else
        true

def programModalitiesAllowed (candidate : List RelationFact) : Bool :=
  candidate.all fun fact => projectProgramMayProduce fact.modality

inductive ClosureTerminal where
  | fixedPoint
  | budgetExhausted
  | blocked
  deriving DecidableEq, Repr

structure ClosureProofDependency where
  conclusionKey : Nat
  premiseKey : Nat
  deriving DecidableEq, Repr

structure AscentClosureCandidate where
  sourceIdentity : Nat
  programIdentity : Nat
  facts : List RelationFact
  nextFacts : List RelationFact
  proofDependencies : List ClosureProofDependency
  terminal : ClosureTerminal
  receiptIdentity : Nat
  deriving DecidableEq, Repr

def admittedFacts (candidate : AscentClosureCandidate) : List RelationFact :=
  candidate.facts.filter fun fact => factualPremise fact.modality

def closureProofDependenciesComplete (candidate : AscentClosureCandidate) : Bool :=
  candidate.facts.all fun fact =>
    fact.modality != .derived ||
      candidate.proofDependencies.any fun dependency =>
        dependency.conclusionKey == fact.key

def admitClosure
    (expectedSource expectedProgram : Nat)
    (independentlyAdmitted : List AscentClosureCandidate)
    (core : List RelationFact)
    (candidate : AscentClosureCandidate) : Option (List RelationFact) :=
  if independentlyAdmitted.contains candidate &&
      candidate.sourceIdentity == expectedSource &&
      candidate.programIdentity == expectedProgram &&
      candidate.receiptIdentity != 0 &&
      candidate.terminal == .fixedPoint &&
      candidate.facts == candidate.nextFacts &&
      closureProofDependenciesComplete candidate &&
      programModalitiesAllowed candidate.facts &&
      agreesWithCore core candidate.facts then
    some (core ++ admittedFacts candidate)
  else
    none

def parserOwner : RelationFact := ⟨1, 10, .parserFact, 100⟩
def derivedMember : RelationFact := ⟨2, 20, .derived, 200⟩
def proposedMeaning : RelationFact := ⟨3, 30, .proposed, 300⟩
def forgedOwner : RelationFact := ⟨1, 99, .derived, 400⟩

def completeCandidate : AscentClosureCandidate :=
  ⟨11, 17, [derivedMember, proposedMeaning], [derivedMember, proposedMeaning],
    [⟨derivedMember.key, parserOwner.key⟩], .fixedPoint, 71⟩

def exhaustedCandidate : AscentClosureCandidate :=
  ⟨11, 17, [derivedMember], [derivedMember],
    [⟨derivedMember.key, parserOwner.key⟩], .budgetExhausted, 72⟩

def conflictingCandidate : AscentClosureCandidate :=
  ⟨11, 17, [forgedOwner], [forgedOwner],
    [⟨forgedOwner.key, parserOwner.key⟩], .fixedPoint, 73⟩

theorem complete_candidate_settles_derived_but_not_proposed :
    admitClosure 11 17 [completeCandidate] [parserOwner] completeCandidate =
      some [parserOwner, derivedMember] := by
  decide

theorem budget_exhaustion_is_not_fixed_point_admission :
    admitClosure 11 17 [exhaustedCandidate] [parserOwner] exhaustedCandidate = none := by
  decide

theorem project_program_conflict_with_core_fails_closed :
    admitClosure 11 17 [conflictingCandidate] [parserOwner] conflictingCandidate = none := by
  decide

theorem self_reported_fixed_point_without_independent_receipt_is_rejected :
    admitClosure 11 17 [] [parserOwner] completeCandidate = none := by
  decide

def incompleteDependencyCandidate : AscentClosureCandidate :=
  { completeCandidate with proofDependencies := [] }

theorem derived_fact_without_computed_dependency_is_rejected :
    admitClosure 11 17 [incompleteDependencyCandidate] [parserOwner]
      incompleteDependencyCandidate = none := by
  decide

theorem successful_admission_preserves_the_complete_core
    (core : List RelationFact)
    (candidate : AscentClosureCandidate)
    (settled : List RelationFact)
    (admittedReceipts : List AscentClosureCandidate)
    (source program : Nat)
    (admitted : admitClosure source program admittedReceipts core candidate = some settled) :
    ∃ programFacts, settled = core ++ programFacts := by
  unfold admitClosure at admitted
  split at admitted
  · simp only [Option.some.injEq] at admitted
    subst settled
    exact ⟨admittedFacts candidate, rfl⟩
  · contradiction

structure GqlShape where
  key : Nat
  value : Nat
  deriving DecidableEq, Repr

def gqlProjection (fact : RelationFact) : GqlShape :=
  ⟨fact.key, fact.value⟩

def directShape : RelationFact := ⟨8, 80, .parserFact, 801⟩
def derivedShape : RelationFact := ⟨8, 80, .derived, 802⟩

theorem equal_gql_shape_does_not_prove_equal_authority :
    gqlProjection directShape = gqlProjection derivedShape ∧
      directShape ≠ derivedShape := by
  decide

structure RelationSignature where
  namespaceId : Nat
  relationId : Nat
  arity : Nat
  deriving DecidableEq, Repr

def signatureCompatible
    (core project : RelationSignature) : Bool :=
  if core.namespaceId == project.namespaceId &&
      core.relationId == project.relationId then
    core.arity == project.arity
  else
    true

def coreDeclares : RelationSignature := ⟨1, 1, 2⟩
def shadowedDeclares : RelationSignature := ⟨1, 1, 3⟩
def projectDeploysTo : RelationSignature := ⟨7, 1, 3⟩

theorem conflicting_relation_signature_cannot_shadow_core :
    signatureCompatible coreDeclares shadowedDeclares = false := by
  decide

theorem project_namespace_can_add_a_relation :
    signatureCompatible coreDeclares projectDeploysTo = true := by
  decide

structure MrrProgramBinding where
  profileSetDigest : Nat
  preludeDigest : Nat
  projectProgramDigest : Nat
  schemeProgramDigest : Nat
  compiledAbiDigest : Option Nat
  mrrBundleIdentity : Nat
  ascentProgramDigest : Nat
  deriving DecidableEq, Repr

def programReady (binding : MrrProgramBinding) : Bool :=
  binding.compiledAbiDigest.isSome

def exactProgramBinding
    (expected actual : MrrProgramBinding) : Bool :=
  expected == actual && programReady actual

def programA : MrrProgramBinding :=
  ⟨11, 12, 13, 14, some 15, 16, 17⟩

def programAfterSchemeEdit : MrrProgramBinding :=
  ⟨11, 12, 13, 24, some 25, 26, 17⟩

def uncompiledProgram : MrrProgramBinding :=
  ⟨11, 12, 13, 14, none, 16, 17⟩

theorem scheme_or_compiled_abi_change_invalidates_program_binding :
    exactProgramBinding programA programAfterSchemeEdit = false := by
  decide

theorem uncompiled_scheme_program_is_not_hot_path_ready :
    programReady uncompiledProgram = false := by
  decide

structure TopologyBinding where
  sourceSnapshotDigest : Nat
  providerCatalogDigest : Nat
  parserCatalogDigest : Nat
  projectProgramDigest : Nat
  mrrBundleIdentity : Nat
  topologyRootDigest : Nat
  activationGeneration : Nat
  deriving DecidableEq, Repr

def exactTopologyBinding
    (expected actual : TopologyBinding) : Bool :=
  expected.sourceSnapshotDigest == actual.sourceSnapshotDigest &&
    expected.providerCatalogDigest == actual.providerCatalogDigest &&
    expected.parserCatalogDigest == actual.parserCatalogDigest &&
    expected.projectProgramDigest == actual.projectProgramDigest &&
    expected.mrrBundleIdentity == actual.mrrBundleIdentity &&
    expected.topologyRootDigest == actual.topologyRootDigest

def topologyBefore : TopologyBinding :=
  ⟨1, 2, 3, 4, 5, 6, 81⟩

def topologyWithStaleSource : TopologyBinding :=
  ⟨9, 2, 3, 4, 5, 6, 81⟩

theorem activation_generation_cannot_prove_topology_freshness :
    topologyBefore.activationGeneration =
        topologyWithStaleSource.activationGeneration ∧
      exactTopologyBinding topologyBefore topologyWithStaleSource = false := by
  decide

def containsFactKey (facts : List RelationFact) (key : Nat) : Bool :=
  facts.any fun fact => fact.key == key

def replaceFacts
    (previous : List RelationFact)
    (removedKeys : List Nat)
    (added : List RelationFact) : List RelationFact :=
  previous.filter (fun fact => !(removedKeys.contains fact.key)) ++ added

def previousFacts : List RelationFact := [parserOwner, derivedMember]

theorem addition_only_update_retains_a_deleted_fact :
    containsFactKey (previousFacts ++ []) derivedMember.key = true := by
  decide

theorem replacement_delta_removes_the_deleted_fact :
    containsFactKey (replaceFacts previousFacts [derivedMember.key] [])
        derivedMember.key = false := by
  decide

structure RelationDelta where
  fromIdentity : Nat
  toIdentity : Nat
  removedKeys : List Nat
  added : List RelationFact
  deriving DecidableEq, Repr

def removalDelta : RelationDelta :=
  ⟨41, 42, [derivedMember.key], []⟩

structure ProofDependency where
  conclusionKey : Nat
  premiseKey : Nat
  deriving DecidableEq, Repr

def invalidationStep
    (dependencies : List ProofDependency)
    (invalid : List Nat) : List Nat :=
  dependencies.foldl
    (fun accumulated dependency =>
      if dependency.premiseKey ∈ accumulated &&
          dependency.conclusionKey ∉ accumulated then
        dependency.conclusionKey :: accumulated
      else
        accumulated)
    invalid

def invalidationClosure
    (fuel : Nat)
    (dependencies : List ProofDependency)
    (invalid : List Nat) : List Nat :=
  match fuel with
  | 0 => invalid
  | fuel + 1 => invalidationClosure fuel dependencies
      (invalidationStep dependencies invalid)

def applyDelta
    (actualIdentity : Nat)
    (previous : List RelationFact)
    (dependencies : List ProofDependency)
    (delta : RelationDelta) : Option (List RelationFact) :=
  if actualIdentity == delta.fromIdentity then
    let invalid := invalidationClosure dependencies.length dependencies delta.removedKeys
    some (replaceFacts previous invalid delta.added)
  else
    none

theorem delta_requires_the_exact_predecessor_identity :
    applyDelta 40 previousFacts [] removalDelta = none ∧
      applyDelta 41 previousFacts [] removalDelta =
        some [parserOwner] := by
  decide

def twoHopDependencies : List ProofDependency :=
  [⟨3, 2⟩, ⟨2, 1⟩]

theorem one_invalidation_pass_does_not_close_two_hops :
    2 ∈ invalidationClosure 1 twoHopDependencies [1] ∧
      3 ∉ invalidationClosure 1 twoHopDependencies [1] := by
  decide

theorem transitive_invalidation_retracts_the_derived_descendant :
    3 ∈ invalidationClosure 2 twoHopDependencies [1] := by
  decide

def transitivePreviousFacts : List RelationFact :=
  [parserOwner, derivedMember, ⟨3, 30, .derived, 300⟩]

def removeParserPremise : RelationDelta :=
  ⟨41, 42, [parserOwner.key], []⟩

theorem incremental_delta_retracts_transitive_derived_descendants :
    applyDelta 41 transitivePreviousFacts twoHopDependencies removeParserPremise =
      some [] := by
  decide

def closureStep (known : List Nat) : List Nat :=
  if 2 ∈ known then known else 2 :: known

def closureRun : Nat → List Nat → List Nat
  | 0, known => known
  | fuel + 1, known => closureRun fuel (closureStep known)

def isFixedPoint (known : List Nat) : Bool :=
  closureStep known == known

theorem fuel_exhaustion_does_not_establish_a_fixed_point :
    closureRun 0 [] = [] ∧ isFixedPoint [] = false := by
  decide

theorem completed_step_reaches_the_fixed_point :
    isFixedPoint (closureRun 1 []) = true := by
  decide

inductive SourceClass where
  | workspaceSource
  | topologyMaterialization
  | runtimeCache
  | gitMetadata
  deriving DecidableEq, Repr

def eligibleTopologyInput : SourceClass → Bool
  | .workspaceSource => true
  | .topologyMaterialization | .runtimeCache | .gitMetadata => false

theorem topology_output_cannot_index_itself :
    eligibleTopologyInput .topologyMaterialization = false := by
  decide

end ASPProof.ProjectTopologyProgram

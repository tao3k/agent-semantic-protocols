-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.SearchLoopRfcLeanSystem

abbrev Digest := Nat

/- Content-addressed parser artifacts deliberately omit generation identity. -/
structure ParserArtifactKey where
  content : Digest
  parser : Digest
  schema : Digest
  deriving DecidableEq, Repr

/- Durable generation restoration needs the complete publication authority. -/
structure GenerationKey where
  workspace : Digest
  canonicalRoot : Digest
  candidate : Digest
  policy : Digest
  sourceRoot : Digest
  generation : Digest
  provider : Digest
  schema : Digest
  parser : Digest
  deriving DecidableEq, Repr

inductive Integrity where
  | verified
  | incomplete
  | corrupt
  deriving DecidableEq, Repr

structure DurableGeneration where
  key : GenerationKey
  integrity : Integrity
  deriving DecidableEq, Repr

def Clean (cached : DurableGeneration) (current : GenerationKey) : Prop :=
  cached.integrity = .verified ∧ cached.key = current

inductive RestoreDecision where
  | reuse
  | rebuildIdentityDrift
  | rejectIncomplete
  | rejectCorrupt
  deriving DecidableEq, Repr

def decideRestore
    (cached : DurableGeneration) (current : GenerationKey) : RestoreDecision :=
  match cached.integrity with
  | .incomplete => .rejectIncomplete
  | .corrupt => .rejectCorrupt
  | .verified =>
      if cached.key = current then .reuse else .rebuildIdentityDrift

theorem restore_reuse_iff_clean
    (cached : DurableGeneration) (current : GenerationKey) :
    decideRestore cached current = .reuse ↔ Clean cached current := by
  cases cached with
  | mk key integrity =>
      cases integrity <;> simp [decideRestore, Clean]

theorem incomplete_generation_never_reuses
    (key current : GenerationKey) :
    decideRestore ⟨key, .incomplete⟩ current = .rejectIncomplete := by
  rfl

theorem corrupt_generation_never_reuses
    (key current : GenerationKey) :
    decideRestore ⟨key, .corrupt⟩ current = .rejectCorrupt := by
  rfl

def parserReusable
    (cached current : ParserArtifactKey) : Prop :=
  cached = current

def baseParserArtifact : ParserArtifactKey :=
  { content := 101, parser := 202, schema := 303 }

def generationA : GenerationKey :=
  { workspace := 1
    canonicalRoot := 2
    candidate := 3
    policy := 4
    sourceRoot := 5
    generation := 6
    provider := 7
    schema := 303
    parser := 202 }

def generationB : GenerationKey :=
  { generationA with generation := 8 }

theorem generation_drift_does_not_destroy_content_artifact :
    generationA ≠ generationB ∧
      parserReusable baseParserArtifact baseParserArtifact := by
  simp [generationA, generationB, parserReusable]

theorem parser_drift_invalidates_content_artifact :
    ¬ parserReusable baseParserArtifact
      { baseParserArtifact with parser := 999 } := by
  simp [parserReusable, baseParserArtifact]

/- The stage language makes the Search/Query algorithm split executable. -/
inductive Stage where
  | identityNormalize
  | generationLookup
  | materializationLookup
  | lexicalAcquire
  | tantivyAcquire
  | ownerIntersect
  | parserGround
  | graphRank
  | exactOwnerLookup
  | selectedMaterialize
  | terminalEncode
  deriving DecidableEq, Repr

def residentStages : List Stage :=
  [.identityNormalize, .generationLookup, .materializationLookup,
   .terminalEncode]

def searchStages : List Stage :=
  [.identityNormalize, .generationLookup, .lexicalAcquire, .tantivyAcquire,
   .ownerIntersect, .parserGround, .graphRank, .terminalEncode]

def exactQueryStages : List Stage :=
  [.identityNormalize, .generationLookup, .exactOwnerLookup,
   .selectedMaterialize, .terminalEncode]

theorem resident_excludes_repository_work :
    ¬ residentStages.contains .lexicalAcquire ∧
    ¬ residentStages.contains .tantivyAcquire ∧
    ¬ residentStages.contains .parserGround := by
  decide

theorem exact_query_excludes_search_acquisition :
    ¬ exactQueryStages.contains .lexicalAcquire ∧
    ¬ exactQueryStages.contains .tantivyAcquire ∧
    ¬ exactQueryStages.contains .ownerIntersect ∧
    ¬ exactQueryStages.contains .graphRank := by
  decide

theorem search_contains_both_acquisition_cores :
    searchStages.contains .lexicalAcquire ∧
    searchStages.contains .tantivyAcquire ∧
    searchStages.contains .ownerIntersect := by
  decide

/- Scheme composition is recursively extensible but every admitted value is a
   finite AST whose execution is bounded independently of syntactic openness. -/
inductive Composition where
  | leaf (stage : Stage)
  | chain (left right : Composition)
  | intersect (left right : Composition)
  deriving DecidableEq, Repr

def Composition.nodeCount : Composition → Nat
  | .leaf _ => 1
  | .chain left right
  | .intersect left right => left.nodeCount + right.nodeCount + 1

def Composition.depth : Composition → Nat
  | .leaf _ => 1
  | .chain left right
  | .intersect left right => Nat.max left.depth right.depth + 1

def Composition.staticWork : Composition → Nat
  | .leaf _ => 1
  | .chain left right => left.staticWork + right.staticWork + 1
  | .intersect left right => left.staticWork + right.staticWork + 2

structure CompositionBudget where
  maxNodes : Nat
  maxDepth : Nat
  maxStaticWork : Nat
  deriving DecidableEq, Repr

def CompositionAdmitted
    (budget : CompositionBudget) (plan : Composition) : Prop :=
  plan.nodeCount ≤ budget.maxNodes ∧
  plan.depth ≤ budget.maxDepth ∧
  plan.staticWork ≤ budget.maxStaticWork

instance compositionAdmittedDecidable
    (budget : CompositionBudget) (plan : Composition) :
    Decidable (CompositionAdmitted budget plan) := by
  unfold CompositionAdmitted
  infer_instance

def compositionTower : Nat → Composition
  | 0 => .leaf .lexicalAcquire
  | level + 1 => .chain (compositionTower level) (.leaf .parserGround)

theorem composition_is_unboundedly_extensible_but_each_value_is_finite
    (level : Nat) :
    (compositionTower level).depth = level + 1 := by
  induction level with
  | zero => rfl
  | succ level inductionHypothesis =>
      simp [compositionTower, Composition.depth, inductionHypothesis]

theorem zero_node_budget_rejects_every_composition
    (plan : Composition) :
    ¬ CompositionAdmitted
        { maxNodes := 0, maxDepth := 0, maxStaticWork := 0 }
        plan := by
  cases plan <;> simp [CompositionAdmitted, Composition.nodeCount]

structure ExpansionAuthority where
  sourceDigest : Digest
  operatorRegistry : Digest
  macroArtifact : Digest
  deriving DecidableEq, Repr

structure ExpandedComposition where
  authority : ExpansionAuthority
  plan : Composition
  deriving DecidableEq, Repr

def expansionReusable
    (cached current : ExpandedComposition) : Prop :=
  cached = current

theorem operator_registry_drift_invalidates_expansion
    (cached : ExpandedComposition) (newRegistry : Digest)
    (drift : newRegistry ≠ cached.authority.operatorRegistry) :
    ¬ expansionReusable cached
      { cached with authority :=
          { cached.authority with operatorRegistry := newRegistry } } := by
  intro reused
  have authorityEqual := congrArg
    (fun value : ExpandedComposition => value.authority.operatorRegistry)
    reused
  exact drift authorityEqual.symm

structure WorkCoefficients where
  normalize : Nat
  generationLookup : Nat
  materializationLookup : Nat
  exactOwnerLookup : Nat
  selectedMaterializePerByte : Nat
  lexicalPerCandidate : Nat
  tantivyPerCandidate : Nat
  intersectPerCandidate : Nat
  groundPerCandidate : Nat
  rankPerCandidate : Nat
  parsePerChangedByte : Nat
  scanPerChangedFile : Nat
  publish : Nat
  encodePerByte : Nat
  deriving DecidableEq, Repr

def residentWork
    (cost : WorkCoefficients) (_corpusFiles outputBytes : Nat) : Nat :=
  cost.normalize + cost.generationLookup + cost.materializationLookup +
    cost.encodePerByte * outputBytes

def exactQueryWork
    (cost : WorkCoefficients) (materializedBytes outputBytes : Nat) : Nat :=
  cost.normalize + cost.generationLookup + cost.exactOwnerLookup +
    cost.selectedMaterializePerByte * materializedBytes +
    cost.encodePerByte * outputBytes

def searchWork
    (cost : WorkCoefficients)
    (rgCandidates tantivyCandidates fusedCandidates groundedCandidates
      outputBytes : Nat) : Nat :=
  cost.normalize + cost.generationLookup +
    cost.lexicalPerCandidate * rgCandidates +
    cost.tantivyPerCandidate * tantivyCandidates +
    cost.intersectPerCandidate * (rgCandidates + tantivyCandidates) +
    cost.groundPerCandidate * fusedCandidates +
    cost.rankPerCandidate * groundedCandidates +
    cost.encodePerByte * outputBytes

def incrementalBuildWork
    (cost : WorkCoefficients) (changedFiles changedBytes : Nat) : Nat :=
  cost.scanPerChangedFile * changedFiles +
    cost.parsePerChangedByte * changedBytes + cost.publish

def fullBuildWork
    (cost : WorkCoefficients) (allFiles allBytes : Nat) : Nat :=
  cost.scanPerChangedFile * allFiles +
    cost.parsePerChangedByte * allBytes + cost.publish

theorem resident_work_is_corpus_independent
    (cost : WorkCoefficients) (leftFiles rightFiles outputBytes : Nat) :
    residentWork cost leftFiles outputBytes =
      residentWork cost rightFiles outputBytes := by
  rfl

theorem zero_delta_has_no_scan_or_parse_work
    (cost : WorkCoefficients) :
    incrementalBuildWork cost 0 0 = cost.publish := by
  simp [incrementalBuildWork]

theorem full_build_is_incremental_at_full_delta
    (cost : WorkCoefficients) (allFiles allBytes : Nat) :
    fullBuildWork cost allFiles allBytes =
      incrementalBuildWork cost allFiles allBytes := by
  rfl

/- Single-flight charges one computation to any non-empty waiter set. -/
def singleFlightComputeWork (waiters computeWork : Nat) : Nat :=
  if waiters = 0 then 0 else computeWork

theorem joiners_do_not_multiply_compute
    (joinedWaiters computeWork : Nat) :
    singleFlightComputeWork (joinedWaiters + 1) computeWork = computeWork := by
  simp [singleFlightComputeWork]

structure ResourceVector where
  cpu : Nat
  bytes : Nat
  memory : Nat
  queue : Nat
  deriving DecidableEq, Repr

def Fits (demand budget : ResourceVector) : Prop :=
  demand.cpu ≤ budget.cpu ∧ demand.bytes ≤ budget.bytes ∧
  demand.memory ≤ budget.memory ∧ demand.queue ≤ budget.queue

instance fitsDecidable
    (demand budget : ResourceVector) : Decidable (Fits demand budget) := by
  unfold Fits
  infer_instance

inductive BudgetDecision where
  | admit
  | reject
  deriving DecidableEq, Repr

def decideBudget
    (demand budget : ResourceVector) : BudgetDecision :=
  if Fits demand budget then .admit else .reject

theorem budget_admission_iff_all_dimensions_fit
    (demand budget : ResourceVector) :
    decideBudget demand budget = .admit ↔ Fits demand budget := by
  simp [decideBudget]

structure LatencyPhases where
  semantic : Nat
  serialization : Nat
  transport : Nat
  clientAdmission : Nat
  deriving DecidableEq, Repr

def endToEndLatency (phases : LatencyPhases) : Nat :=
  phases.semantic + phases.serialization + phases.transport +
    phases.clientAdmission

theorem end_to_end_is_phase_sum (phases : LatencyPhases) :
    endToEndLatency phases =
      phases.semantic + phases.serialization + phases.transport +
        phases.clientAdmission := by
  rfl

end ASPProof.SearchLoopRfcLeanSystem

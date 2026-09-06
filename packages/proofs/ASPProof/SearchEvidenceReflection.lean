import Std
import ASPProof.SearchRouteDecisionSufficientInspect

/-!
Repository-integrated design research model, not a proof about the current ASP implementation.
Finite identities below are abstract values, not real content digests.
No claims about model reasoning quality, token optimality, or Org parser conformance.
-/

namespace ASPProof.SearchEvidenceReflection

inductive Node where
  | registryImpl | refresh | publish | storeLoad | cacheLoad
  deriving DecidableEq, Repr

inductive Relation where
  | memberOf | calls | declares
  deriving DecidableEq, Repr

structure Fact where
  subject : Node
  relation : Relation
  target : Node
  deriving DecidableEq, Repr

def membership : Fact := ⟨.refresh, .memberOf, .registryImpl⟩
def storeCall : Fact := ⟨.refresh, .calls, .storeLoad⟩
def cacheCall : Fact := ⟨.refresh, .calls, .cacheLoad⟩

/- R01: a body-dependent fact is not licensed by topology equality. -/

inductive TopologyIdentity where
  | registryLayout | otherLayout
  deriving DecidableEq, Repr

inductive ContentIdentity where
  | storeImplementation | cacheImplementation
  deriving DecidableEq, Repr

inductive ResolverIdentity where
  | catalogA | catalogB
  deriving DecidableEq, Repr

structure Binding where
  topology : TopologyIdentity
  content : ContentIdentity
  resolver : ResolverIdentity
  deriving DecidableEq, Repr

def before : Binding := ⟨.registryLayout, .storeImplementation, .catalogA⟩
def afterBodyEdit : Binding := ⟨.registryLayout, .cacheImplementation, .catalogA⟩
def afterResolverEdit : Binding := ⟨.registryLayout, .storeImplementation, .catalogB⟩

def admitExact (expected actual : Binding) : Bool := expected == actual

theorem topology_equality_does_not_imply_exact_binding :
    before.topology = afterBodyEdit.topology ∧
    admitExact before afterBodyEdit = false := by decide

theorem resolver_change_is_not_licensed_by_same_content :
    before.content = afterResolverEdit.content ∧
    admitExact before afterResolverEdit = false := by decide

def reusableFact (fact : Fact) (expected actual : Binding) : Bool :=
  match fact.relation with
  | .memberOf | .declares => expected.topology == actual.topology
  | .calls => admitExact expected actual

theorem topology_fact_reuse_is_not_body_reuse :
    reusableFact membership before afterBodyEdit = true ∧
    reusableFact storeCall before afterBodyEdit = false := by decide

theorem exact_admission_sound (expected actual : Binding)
    (h : admitExact expected actual = true) : expected = actual := by
  simpa [admitExact] using h

/- R02: names of discovery seeds must not be canonical owner identities. -/

def ownerOf : Node → Option Node
  | .refresh | .publish => some .registryImpl
  | _ => none

theorem distinct_seeds_share_one_owner :
    Node.refresh ≠ Node.publish ∧ ownerOf .refresh = ownerOf .publish := by decide

def canonicalOwnerSelection (seed : Node) : Option Node := ownerOf seed

theorem owner_selection_independent_of_seed (a b : Node)
    (h : ownerOf a = ownerOf b) :
    canonicalOwnerSelection a = canonicalOwnerSelection b := h

/- A raw syntax tree can retain both cfg branches. Active membership cannot
   be reused as if it were the same question. Topology identities here are
   assumed parser/grammar validated, not hashes of display names. -/
inductive Configuration where
  | unix | windows
  deriving DecidableEq, Repr

inductive StructuralView where
  | rawSyntax | activeConfiguration
  deriving DecidableEq, Repr

def reuseStructure (view : StructuralView) (oldTopology newTopology : TopologyIdentity)
    (oldConfig newConfig : Configuration) : Bool :=
  oldTopology == newTopology &&
    match view with
    | .rawSyntax => true
    | .activeConfiguration => oldConfig == newConfig

theorem raw_and_active_structure_have_distinct_dependencies :
    reuseStructure .rawSyntax .registryLayout .registryLayout .unix .windows = true ∧
    reuseStructure .activeConfiguration .registryLayout .registryLayout .unix .windows = false :=
  by decide

structure ImplIdentity where
  displayName : String
  instanceId : Nat
  configuration : Configuration
  deriving DecidableEq, Repr

def unixImpl : ImplIdentity := ⟨"Registry", 0, .unix⟩
def windowsImpl : ImplIdentity := ⟨"Registry", 1, .windows⟩

theorem same_type_name_does_not_identify_an_impl :
    unixImpl.displayName = windowsImpl.displayName ∧ unixImpl ≠ windowsImpl := by decide

/- R03: bounded projections cannot answer arbitrary unseen future questions.
    A question here is only fact membership; this already suffices for the
    impossibility witness, without making assumptions about LLM behavior. -/

def projectFacts (requested source : List Fact) : List Fact :=
  source.filter (fun f => decide (f ∈ requested))

def knows (facts : List Fact) (question : Fact) : Prop := question ∈ facts

theorem requested_fact_is_preserved (requested source : List Fact) (q : Fact)
    (hq : q ∈ requested) :
    knows (projectFacts requested source) q ↔ knows source q := by
  simp [knows, projectFacts, hq]

theorem omitted_fact_has_indistinguishable_worlds
    (requested : List Fact) (q : Fact) (hq : q ∉ requested) :
    projectFacts requested [] = projectFacts requested [q] ∧
    ¬ knows [] q ∧ knows [q] q := by
  simp [projectFacts, knows, hq]

theorem no_decoder_can_recover_an_omitted_fact (requested : List Fact) (q : Fact)
    (hq : q ∉ requested) :
    ¬ ∃ decode : List Fact → Bool, ∀ world,
      decode (projectFacts requested world) = decide (q ∈ world) := by
  rintro ⟨decode, correct⟩
  have empty := correct []
  have singleton := correct [q]
  have same := (omitted_fact_has_indistinguishable_worlds requested q hq).1
  rw [← same] at singleton
  simp only [List.mem_nil_iff, decide_false] at empty
  simp at singleton
  rw [empty] at singleton
  cases singleton

theorem skeleton_cannot_decide_implementation_call :
    projectFacts [membership] [membership, storeCall] =
      projectFacts [membership] [membership, cacheCall] ∧
    knows [membership, storeCall] storeCall ∧
    ¬ knows [membership, cacheCall] storeCall := by
  unfold knows
  decide

/- R04: omitted and absent must stay distinct.  `covered` is the certified
    question domain, not the list of positive hits or a global complete bit. -/

inductive Answer where
  | supported | ruledOut | unknown
  deriving DecidableEq, Repr

structure View where
  positive : List Fact
  covered : List Fact
  deriving DecidableEq, Repr

def answer (view : View) (q : Fact) : Answer :=
  if q ∈ view.positive then .supported
  else if q ∈ view.covered then .ruledOut
  else .unknown

def projectView (requested : List Fact) (source : View) : View :=
  ⟨projectFacts requested source.positive, projectFacts requested source.covered⟩

def ValidView (truth : List Fact) (view : View) : Prop :=
  (∀ f, f ∈ view.positive → f ∈ truth) ∧
  (∀ f, f ∈ view.covered → (f ∈ view.positive ↔ f ∈ truth))

theorem projection_preserves_declared_question (requested : List Fact)
    (source : View) (q : Fact) (hq : q ∈ requested) :
    answer (projectView requested source) q = answer source q := by
  simp [answer, projectView, projectFacts, hq]

theorem projection_preserves_validity (requested truth : List Fact) (source : View)
    (h : ValidView truth source) : ValidView truth (projectView requested source) := by
  constructor
  · intro f hf
    have hf' : f ∈ source.positive ∧ f ∈ requested := by
      simpa [projectView, projectFacts] using hf
    exact h.1 f hf'.1
  · intro f hf
    have hf' : f ∈ source.covered ∧ f ∈ requested := by
      simpa [projectView, projectFacts] using hf
    simpa [projectView, projectFacts, hf'.2] using h.2 f hf'.1

theorem filtered_positive_with_unfiltered_coverage_is_false_absence :
    answer ⟨[membership], [membership, storeCall]⟩ storeCall = .ruledOut ∧
    ¬ ValidView [membership, storeCall] ⟨[membership], [membership, storeCall]⟩ := by
  constructor
  · decide
  · intro h
    have impossible : storeCall ∈ [membership] :=
      (h.2 storeCall (by simp)).mpr (by simp)
    exact (by decide : storeCall ∉ [membership]) impossible

theorem honest_partial_directory_is_unknown :
    answer (projectView [membership] ⟨[membership, storeCall], [membership, storeCall]⟩)
      storeCall = .unknown := by decide

/- Coverage is certified complete evidence, not a promise that a question
   was attempted. The abstract certificate must be established by an actual
   producer/validator in any implementation refinement. -/
theorem ruled_out_requires_sound_complete_coverage (truth : List Fact) (view : View)
    (q : Fact) (valid : ValidView truth view) (h : answer view q = .ruledOut) :
    q ∉ truth := by
  by_cases positive : q ∈ view.positive
  · simp [answer, positive] at h
  by_cases covered : q ∈ view.covered
  · intro present
    exact positive ((valid.2 q covered).mpr present)
  · simp [answer, positive, covered] at h

structure CertifiedView (truth : List Fact) where
  view : View
  sound : ValidView truth view

def certifiedAnswer {truth : List Fact} (view : CertifiedView truth) (q : Fact) : Answer :=
  answer view.view q

theorem certified_negative_is_not_merely_missing (truth : List Fact)
    (view : CertifiedView truth) (q : Fact) (h : certifiedAnswer view q = .ruledOut) :
    q ∉ truth :=
  ruled_out_requires_sound_complete_coverage truth view.view q view.sound h


/- Reuse the existing decision-correctness contract rather than defining a
   parallel criterion. Zero is an unmeasured cost placeholder, not a claim
   about the byte or token cost of a renderer. -/
def declaredQuestionSurface (requested : List Fact) (q : Fact) :
    SearchRouteDecisionSufficientInspect.InspectSurface View View Answer :=
  { tokenCost := 0
    project := projectView requested
    decode := fun view => answer view q }

theorem declared_projection_is_decision_correct (requested : List Fact) (q : Fact)
    (hq : q ∈ requested) :
    SearchRouteDecisionSufficientInspect.DecisionCorrect
      (declaredQuestionSurface requested q) (fun view => answer view q) := by
  intro view
  exact projection_preserves_declared_question requested view q hq

/- R05: collector count is not independent evidence count.
    Deduplicate source occurrences while retaining every collector attribution. -/

inductive Collector where
  | rg | lexical | path
  deriving DecidableEq, Repr

inductive Occurrence where
  | firstLiteral | secondLiteral
  deriving DecidableEq, Repr

structure Witness where
  fact : Fact
  occurrence : Occurrence
  binding : Binding
  deriving DecidableEq, Repr

structure Observation where
  witness : Witness
  collector : Collector
  deriving DecidableEq, Repr

def uniqueWitnesses (observations : List Observation) : List Witness :=
  (observations.map (·.witness)).eraseDups

def producersFor (w : Witness) (observations : List Observation) : List Collector :=
  ((observations.filter (fun o => o.witness == w)).map (·.collector)).eraseDups

def occurrenceA : Witness := ⟨storeCall, .firstLiteral, before⟩
def occurrenceB : Witness := ⟨storeCall, .secondLiteral, before⟩
def repeatedObservation : List Observation :=
  [⟨occurrenceA, .rg⟩, ⟨occurrenceA, .lexical⟩]

theorem two_collectors_can_be_one_witness :
    repeatedObservation.length = 2 ∧
    (uniqueWitnesses repeatedObservation).length = 1 ∧
    producersFor occurrenceA repeatedObservation = [.rg, .lexical] := by decide

theorem same_fact_with_two_occurrences_retains_two_witnesses :
    (uniqueWitnesses [⟨occurrenceA, .rg⟩, ⟨occurrenceB, .rg⟩]).length = 2 := by decide

theorem same_occurrence_with_different_binding_is_not_deduplicated :
    (uniqueWitnesses [⟨occurrenceA, .rg⟩,
      ⟨⟨storeCall, .firstLiteral, afterBodyEdit⟩, .lexical⟩]).length = 2 := by decide

theorem witness_membership_preserved (observations : List Observation) (w : Witness) :
    w ∈ uniqueWitnesses observations ↔ ∃ o ∈ observations, o.witness = w := by
  simp [uniqueWitnesses]

/- R06: deletion-minimal is not globally minimum-cost, and is not a proof of
    agent readability. Costs are declared abstract units, not token estimates. -/

inductive Obligation where
  | selectMember | explainOwner
  deriving DecidableEq, Repr

structure Atom where
  provides : List Obligation
  cost : Nat
  deriving DecidableEq, Repr

def sufficient (required : List Obligation) (packet : List Atom) : Bool :=
  required.all (fun r => packet.any (fun a => a.provides.contains r))

def packetCost (packet : List Atom) : Nat :=
  packet.foldl (fun total atom => total + atom.cost) 0

def deletionMinimal (required : List Obligation) (packet : List Atom) : Bool :=
  sufficient required packet &&
    packet.all (fun atom => !(sufficient required (packet.erase atom)))

def obligations : List Obligation := [.selectMember, .explainOwner]
def separateAtoms : List Atom := [⟨[.selectMember], 2⟩, ⟨[.explainOwner], 2⟩]
def sharedAtom : List Atom := [⟨[.selectMember, .explainOwner], 3⟩]

theorem deletion_minimal_need_not_minimize_cost :
    deletionMinimal obligations separateAtoms = true ∧
    sufficient obligations sharedAtom = true ∧
    packetCost sharedAtom < packetCost separateAtoms := by decide

theorem shortest_packet_need_not_be_sufficient :
    packetCost ([] : List Atom) < packetCost sharedAtom ∧
    sufficient obligations [] = false := by decide

/- R07: previously sent evidence is not necessarily retained by the client.
    Exact handles include dependency identity. Absence of a replay witness
    must not produce a reference-only success. -/

structure EvidenceKey where
  witness : Witness
  deriving DecidableEq, Repr

def keyBefore : EvidenceKey := ⟨occurrenceA⟩
def keyAfter : EvidenceKey := ⟨⟨storeCall, .firstLiteral, afterBodyEdit⟩⟩

def canReplay (key : EvidenceKey) (retained retrievable : List EvidenceKey) : Bool :=
  retained.contains key || retrievable.contains key

theorem previously_sent_does_not_imply_replayability :
    keyBefore ∈ [keyBefore] ∧ canReplay keyBefore [] [] = false := by decide

theorem retrievable_reference_survives_context_eviction :
    canReplay keyBefore [] [keyBefore] = true := by decide

theorem old_reference_does_not_authorize_new_content :
    canReplay keyAfter [keyBefore] [keyBefore] = false := by decide

structure ArchivedEvidence where
  key : EvidenceKey
  fact : Fact
  deriving DecidableEq, Repr

def resolveReference (requested : EvidenceKey) (archive : List ArchivedEvidence) : Option Fact := do
  let record ← archive.find? (fun record => record.key == requested)
  if record.fact == requested.witness.fact then some record.fact else none

def currentDecision (requested : EvidenceKey) (current : Binding)
    (archive : List ArchivedEvidence) : Option Fact :=
  if admitExact requested.witness.binding current then resolveReference requested archive
  else none

def historyArchive : List ArchivedEvidence := [⟨keyBefore, storeCall⟩]

theorem historical_replay_and_current_admission_are_distinct :
    resolveReference keyBefore historyArchive = some storeCall ∧
    currentDecision keyBefore afterBodyEdit historyArchive = none := by decide

theorem forged_reference_payload_fails_closed :
    resolveReference keyBefore [⟨keyBefore, cacheCall⟩] = none := by decide

theorem unresolved_history_does_not_become_a_fact :
    resolveReference keyBefore [] = none := by decide

/- R08: reachability-preserving graph reduction can erase direct-call evidence. -/

abbrev Edge := Node × Node

def direct (edges : List Edge) (a b : Node) : Bool := edges.contains (a, b)

def reachableWithin : Nat → List Edge → Node → Node → Bool
  | 0, _, a, b => a == b
  | n + 1, edges, a, b =>
      a == b || edges.any (fun e => e.1 == a && reachableWithin n edges e.2 b)

def callGraph : List Edge :=
  [(.refresh, .publish), (.publish, .storeLoad), (.refresh, .storeLoad)]
def reducedGraph : List Edge := [(.refresh, .publish), (.publish, .storeLoad)]

theorem reachability_does_not_preserve_direct_calls :
    reachableWithin 3 callGraph .refresh .storeLoad = true ∧
    reachableWithin 3 reducedGraph .refresh .storeLoad = true ∧
    direct callGraph .refresh .storeLoad ≠ direct reducedGraph .refresh .storeLoad := by
  decide

/- R09: raw source may itself contain an Org delimiter. This is an abstract
    framing countermodel, not a verified implementation of Org escaping. -/

inductive RawLine where
  | sourceText | orgEndDelimiter
  deriving DecidableEq, Repr

def unsafeDelimitedRead (source : List RawLine) : List RawLine :=
  source.takeWhile (fun line => line != .orgEndDelimiter)

theorem unescaped_delimiter_loses_native_content :
    unsafeDelimitedRead [.sourceText, .orgEndDelimiter, .sourceText] ≠
      [.sourceText, .orgEndDelimiter, .sourceText] := by decide

/- R10: a fact-backed direct member choice does not depend on a prior skeleton
    request. Discovery history is intentionally not an admission parameter. -/

def queryAdmitted (selected available : Node) (expected actual : Binding) : Bool :=
  selected == available && admitExact expected actual

theorem direct_member_query_is_legal :
    queryAdmitted .refresh .refresh before before = true := by decide

theorem different_target_cannot_be_silently_expanded :
    queryAdmitted .refresh .registryImpl before before = false := by decide

def checks : List (String × Bool) :=
  [ ("R01-body-binding", decide (before.topology = afterBodyEdit.topology) &&
      !admitExact before afterBodyEdit)
  , ("R01-resolver-binding", !admitExact before afterResolverEdit)
  , ("R01-topology-reuse", reusableFact membership before afterBodyEdit &&
      !reusableFact storeCall before afterBodyEdit)
  , ("R02-owner-identity", decide (ownerOf .refresh = ownerOf .publish))
  , ("R02-cfg-view", reuseStructure .rawSyntax .registryLayout .registryLayout .unix .windows &&
      !reuseStructure .activeConfiguration .registryLayout .registryLayout .unix .windows)
  , ("R02-distinct-impl", decide (unixImpl ≠ windowsImpl))
  , ("R03-future-question", decide (projectFacts [membership] [membership, storeCall] =
      projectFacts [membership] [membership, cacheCall]))
  , ("R04-coverage", answer (projectView [membership]
      ⟨[membership, storeCall], [membership, storeCall]⟩) storeCall == .unknown)
  , ("R05-dedup", (uniqueWitnesses repeatedObservation).length == 1)
  , ("R05-provenance", (producersFor occurrenceA repeatedObservation).length == 2)
  , ("R05-cross-binding", (uniqueWitnesses [⟨occurrenceA, .rg⟩,
      ⟨⟨storeCall, .firstLiteral, afterBodyEdit⟩, .lexical⟩]).length == 2)
  , ("R06-not-optimal", deletionMinimal obligations separateAtoms &&
      decide (packetCost sharedAtom < packetCost separateAtoms))
  , ("R07-replay-missing", !canReplay keyBefore [] [])
  , ("R07-replay-stale", !canReplay keyAfter [keyBefore] [keyBefore])
  , ("R07-history-vs-current", decide (resolveReference keyBefore historyArchive = some storeCall) &&
      decide (currentDecision keyBefore afterBodyEdit historyArchive = none))
  , ("R07-forged-payload", decide (resolveReference keyBefore [⟨keyBefore, cacheCall⟩] = none))
  , ("R07-unresolved-history", decide (resolveReference keyBefore [] = none))
  , ("R08-edge-reduction", reachableWithin 3 reducedGraph .refresh .storeLoad &&
      !direct reducedGraph .refresh .storeLoad)
  , ("R09-framing", decide (unsafeDelimitedRead
      [.sourceText, .orgEndDelimiter, .sourceText] ≠
      [.sourceText, .orgEndDelimiter, .sourceText]))
  , ("R10-direct-query", queryAdmitted .refresh .refresh before before)
  ]

theorem all_reflection_checks_pass : checks.all (·.2) = true := by decide

end ASPProof.SearchEvidenceReflection

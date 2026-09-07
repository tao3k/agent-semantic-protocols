-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.HookPolicyAntiHardcoding

/-- Every Hook policy dimension is parser/config projected. The kernel is
parametric in the value types and therefore cannot enumerate concrete action,
extension, profile, or tool labels. -/
structure ActionFacts
    (ActionKind LanguageExtension CommandProfile ToolSurface : Type) where
  actions : List ActionKind
  languageExtensions : List LanguageExtension
  commandProfiles : List CommandProfile
  toolSurfaces : List ToolSurface
  wrappedStagesPresent : Bool

/-- Immutable facts supplied by the Host invocation boundary. The shell parser
cannot rewrite these fields. -/
structure HostInvocationFact
    (ActionKind ToolName Payload InvocationSource : Type) where
  action : ActionKind
  toolName : ToolName
  payload : Payload
  invocationSource : Option InvocationSource

/-- One parser- or Host-proven capability. It deliberately has no authority,
environment, confidence, review, or executable-name dimension. -/
structure SemanticCapability (CapabilityKind Evidence : Type) where
  kind : CapabilityKind
  evidence : Evidence

/-- A classified policy subject, kept orthogonal to capability facts. -/
structure Subject (SubjectKind SubjectValue : Type) where
  kind : SubjectKind
  value : SubjectValue

/-- The v2 Hook fact product: Host facts, semantic capabilities, and subjects
are separate layers. Profiles and extensions remain declarative routing axes. -/
structure SemanticActionFacts
    (ActionKind ToolName Payload InvocationSource CapabilityKind Evidence
      SubjectKind SubjectValue LanguageExtension CommandProfile ToolSurface : Type) where
  hostInvocation : HostInvocationFact ActionKind ToolName Payload InvocationSource
  capabilities : List (SemanticCapability CapabilityKind Evidence)
  subjects : List (Subject SubjectKind SubjectValue)
  languageExtensions : List LanguageExtension
  commandProfiles : List CommandProfile
  toolSurfaces : List ToolSurface
  wrappedStagesPresent : Bool

/-- A rule is a predicate over normalized facts. Conjunction, disjunction, and
negation are policy composition, not branches over raw Host strings. -/
structure PolicyRule (Facts : Type) where
  holds : Facts → Prop

structure PolicySnapshot (Facts : Type) where
  denyRules : List (PolicyRule Facts)

def denies (snapshot : PolicySnapshot Facts) (facts : Facts) : Prop :=
  ∃ rule ∈ snapshot.denyRules, rule.holds facts

/-- Host envelopes are normalized before entering the policy kernel. -/
def evaluate
    (normalize : RawEnvelope → Facts)
    (snapshot : PolicySnapshot Facts)
    (envelope : RawEnvelope) : Prop :=
  denies snapshot (normalize envelope)

/-- Generic all-of composition used by multi-dimensional rules such as
ReadAction + provider extension or wrapped-mode + command profile. -/
def allOf {Facts : Type} (predicates : List (Facts → Prop)) (facts : Facts) : Prop :=
  ∀ predicate ∈ predicates, predicate facts

theorem all_of_requires_every_policy_component
    {Facts : Type}
    (predicates : List (Facts → Prop))
    (facts : Facts)
    (allMatch : allOf predicates facts)
    (predicate : Facts → Prop)
    (member : predicate ∈ predicates) :
    predicate facts :=
  allMatch predicate member

/-- A performance shard is minted from the rule that the complete normalized
policy already proved applicable. It does not invent another admission rule. -/
def winningRuleShard (rule : PolicyRule Facts) : PolicySnapshot Facts where
  denyRules := [rule]

theorem winning_rule_shard_preserves_denial
    (snapshot : PolicySnapshot Facts)
    (facts : Facts)
    (rule : PolicyRule Facts)
    (member : rule ∈ snapshot.denyRules)
    (matched : rule.holds facts) :
    denies snapshot facts ∧ denies (winningRuleShard rule) facts := by
  constructor
  · exact ⟨rule, member, matched⟩
  · exact ⟨rule, by simp [winningRuleShard], matched⟩

/-- This is the global anti-hardcoding boundary: raw envelopes that normalize
to the same facts are observationally indistinguishable to every Hook policy. -/
theorem normalized_facts_determine_every_policy_decision
    (normalize : RawEnvelope → Facts)
    (snapshot : PolicySnapshot Facts)
    (left right : RawEnvelope)
    (sameFacts : normalize left = normalize right) :
    evaluate normalize snapshot left ↔ evaluate normalize snapshot right := by
  simp [evaluate, sameFacts]

theorem semantic_action_facts_determine_every_policy_decision
    {ActionKind ToolName Payload InvocationSource CapabilityKind Evidence
      SubjectKind SubjectValue LanguageExtension CommandProfile ToolSurface : Type}
    (normalize : RawEnvelope →
      SemanticActionFacts ActionKind ToolName Payload InvocationSource CapabilityKind Evidence
        SubjectKind SubjectValue LanguageExtension CommandProfile ToolSurface)
    (snapshot : PolicySnapshot
      (SemanticActionFacts ActionKind ToolName Payload InvocationSource CapabilityKind Evidence
        SubjectKind SubjectValue LanguageExtension CommandProfile ToolSurface))
    (left right : RawEnvelope)
    (sameFacts : normalize left = normalize right) :
    evaluate normalize snapshot left ↔ evaluate normalize snapshot right := by
  exact normalized_facts_determine_every_policy_decision
    normalize snapshot left right sameFacts

/-- Relabeling any raw wrapper/tool/provider representation is harmless when
the parser/config projection is unchanged. This quantifies over arbitrary raw
and fact types rather than an executable-name enum. -/
theorem arbitrary_raw_relabeling_is_policy_invariant
    (normalizeA : RawA → Facts)
    (normalizeB : RawB → Facts)
    (snapshot : PolicySnapshot Facts)
    (left : RawA)
    (right : RawB)
    (sameFacts : normalizeA left = normalizeB right) :
    evaluate normalizeA snapshot left ↔ evaluate normalizeB snapshot right := by
  simp [evaluate, sameFacts]

/-- A raw-label admission gate is outside the kernel and can disagree for two
envelopes with identical normalized facts. -/
def rawNameGatedEvaluation
    (rawAdmitted : RawEnvelope → Prop)
    (normalize : RawEnvelope → Facts)
    (snapshot : PolicySnapshot Facts)
    (envelope : RawEnvelope) : Prop :=
  rawAdmitted envelope ∧ evaluate normalize snapshot envelope

theorem raw_name_gate_violates_policy_extensibility
    (rawAdmitted : RawEnvelope → Prop)
    (normalize : RawEnvelope → Facts)
    (snapshot : PolicySnapshot Facts)
    (known novel : RawEnvelope)
    (sameFacts : normalize known = normalize novel)
    (knownAdmitted : rawAdmitted known)
    (novelRejected : ¬ rawAdmitted novel)
    (policyDenies : evaluate normalize snapshot known) :
    rawNameGatedEvaluation rawAdmitted normalize snapshot known ∧
      ¬ rawNameGatedEvaluation rawAdmitted normalize snapshot novel ∧
      evaluate normalize snapshot novel := by
  constructor
  · exact ⟨knownAdmitted, policyDenies⟩
  constructor
  · intro gated
    exact novelRejected gated.1
  · exact (normalized_facts_determine_every_policy_decision
      normalize snapshot known novel sameFacts).mp policyDenies

abbrev ExampleFacts := ActionFacts String String String String

def testingFacts : ExampleFacts :=
  { actions := ["execute"]
    languageExtensions := ["rs"]
    commandProfiles := ["testing"]
    toolSurfaces := ["shell-command"]
    wrappedStagesPresent := true }

def testingRule : PolicyRule ExampleFacts where
  holds := fun facts =>
    "execute" ∈ facts.actions ∧
    "testing" ∈ facts.commandProfiles ∧
    facts.wrappedStagesPresent = true

def testingSnapshot : PolicySnapshot ExampleFacts where
  denyRules := [testingRule]

def normalizeExample (_raw : String) : ExampleFacts := testingFacts

def readExtensionFacts (extension : String) : ExampleFacts :=
  { actions := ["read"]
    languageExtensions := [extension]
    commandProfiles := []
    toolSurfaces := ["read-action"]
    wrappedStagesPresent := false }

def readExtensionRule (extension : String) : PolicyRule ExampleFacts where
  holds := fun facts =>
    "read" ∈ facts.actions ∧ extension ∈ facts.languageExtensions

/-- Provider registrations can supply any extension value; the same composed
rule constructor matches it without a Rust-side extension name set. -/
theorem every_provider_extension_uses_the_same_policy_constructor
    (extension : String) :
    denies { denyRules := [readExtensionRule extension] }
      (readExtensionFacts extension) := by
  simp [denies, readExtensionRule, readExtensionFacts]

def wrappedProfileFacts (profile : String) : ExampleFacts :=
  { actions := ["execute"]
    languageExtensions := []
    commandProfiles := [profile]
    toolSurfaces := ["shell-command"]
    wrappedStagesPresent := true }

def wrappedProfileRule (profile : String) : PolicyRule ExampleFacts where
  holds := fun facts =>
    profile ∈ facts.commandProfiles ∧ facts.wrappedStagesPresent = true

/-- Command profiles are equally parametric: wrapped-mode composes with any
serialized profile value and does not enumerate testing/build tools in code. -/
theorem every_command_profile_uses_the_same_policy_constructor
    (profile : String) :
    denies { denyRules := [wrappedProfileRule profile] }
      (wrappedProfileFacts profile) := by
  simp [denies, wrappedProfileRule, wrappedProfileFacts]

structure StructuredProjectionBudget where
  maxSliceItems : Nat

def FiniteSlice (budget : StructuredProjectionBudget) (start finish : Nat) : Prop :=
  start ≤ finish ∧ finish - start ≤ budget.maxSliceItems

/-- Slice admission is parameterized by serialized policy capacity. The kernel
does not enumerate jq expressions, document names, or language registries. -/
theorem finite_structured_slice_is_config_bounded
    (budget : StructuredProjectionBudget)
    (start finish : Nat)
    (ordered : start ≤ finish)
    (withinBudget : finish - start ≤ budget.maxSliceItems) :
    FiniteSlice budget start finish :=
  ⟨ordered, withinBudget⟩

/-- An open-ended slice cannot manufacture the missing finite endpoint from a
policy budget; callers must supply the parsed endpoint before admission. -/
theorem structured_slice_admission_requires_an_endpoint
    (budget : StructuredProjectionBudget)
    (start : Nat) :
    (∃ finish, FiniteSlice budget start finish) → ∃ finish, start ≤ finish := by
  intro bounded
  rcases bounded with ⟨finish, ordered, _⟩
  exact ⟨finish, ordered⟩

/-- The abstract combination plan is config-owned. It carries policy axes but
no Host JSON, tool spelling, or shell executable. -/
structure ConfigCoveragePlan
    (Extension CommandPrefix : Type) where
  extensions : List Extension
  readPrefixes : List CommandPrefix
  maxWrapperDepth : Nat
  directEnvelopeCount : Nat
  shellEnvelopeCount : Nat
  negativeExtensionMutation : Bool

inductive CoveragePolarity where
  | black
  | white
  deriving DecidableEq

inductive CoverageSurface where
  | direct
  | shell
  deriving DecidableEq

structure ConfigCoverageCase
    (Extension CommandPrefix : Type) where
  providerExtension : Extension
  observedExtension : Extension
  commandPrefix : Option CommandPrefix
  surface : CoverageSurface
  polarity : CoveragePolarity
  envelopeSlot : Nat
  wrapperDepth : Nat

def directCoverageCases
    (plan : ConfigCoveragePlan Extension CommandPrefix)
    (mutate : Extension → Extension) :
    List (ConfigCoverageCase Extension CommandPrefix) :=
  plan.extensions.flatMap fun extension =>
    (List.range plan.directEnvelopeCount).flatMap fun envelopeSlot =>
      [ { providerExtension := extension, observedExtension := extension,
          commandPrefix := none,
          surface := .direct, polarity := .black,
          envelopeSlot := envelopeSlot, wrapperDepth := 0 },
        { providerExtension := extension, observedExtension := mutate extension,
          commandPrefix := none,
          surface := .direct, polarity := .white,
          envelopeSlot := envelopeSlot, wrapperDepth := 0 } ]

def shellCoverageCases
    (plan : ConfigCoveragePlan Extension CommandPrefix)
    (mutate : Extension → Extension) :
    List (ConfigCoverageCase Extension CommandPrefix) :=
  plan.extensions.flatMap fun extension =>
    plan.readPrefixes.flatMap fun commandProfile =>
      (List.range plan.shellEnvelopeCount).flatMap fun envelopeSlot =>
        (List.range (plan.maxWrapperDepth + 1)).flatMap fun wrapperDepth =>
          [ { providerExtension := extension, observedExtension := extension,
              commandPrefix := some commandProfile,
              surface := .shell, polarity := .black,
              envelopeSlot := envelopeSlot, wrapperDepth := wrapperDepth },
            { providerExtension := extension, observedExtension := mutate extension,
              commandPrefix := some commandProfile,
              surface := .shell, polarity := .white,
              envelopeSlot := envelopeSlot, wrapperDepth := wrapperDepth } ]

/-- Every provider extension and every declared Host-envelope slot has both a
positive and mechanically mutated negative config witness before Hook code is
allowed to materialize an envelope. -/
theorem config_auto_covers_every_direct_extension_slot
    [DecidableEq Extension]
    [DecidableEq CommandPrefix]
    (plan : ConfigCoveragePlan Extension CommandPrefix)
    (mutate : Extension → Extension)
    (extension : Extension)
    (extensionMember : extension ∈ plan.extensions)
    (slot : Nat)
    (slotBound : slot < plan.directEnvelopeCount) :
    { providerExtension := extension, observedExtension := extension,
      commandPrefix := none,
      surface := CoverageSurface.direct, polarity := CoveragePolarity.black,
      envelopeSlot := slot, wrapperDepth := 0 } ∈ directCoverageCases plan mutate ∧
    { providerExtension := extension, observedExtension := mutate extension,
      commandPrefix := none,
      surface := CoverageSurface.direct, polarity := CoveragePolarity.white,
      envelopeSlot := slot, wrapperDepth := 0 } ∈ directCoverageCases plan mutate := by
  constructor
  · apply List.mem_flatMap.mpr
    refine ⟨extension, extensionMember, ?_⟩
    apply List.mem_flatMap.mpr
    refine ⟨slot, List.mem_range.mpr slotBound, by simp⟩
  · apply List.mem_flatMap.mpr
    refine ⟨extension, extensionMember, ?_⟩
    apply List.mem_flatMap.mpr
    refine ⟨slot, List.mem_range.mpr slotBound, by simp⟩

/-- The same config-auto cross product covers every shell prefix, Host envelope
slot, wrapper depth, provider extension, and mechanically generated white case. -/
theorem config_auto_covers_every_shell_policy_combination
    [DecidableEq Extension]
    [DecidableEq CommandPrefix]
    (plan : ConfigCoveragePlan Extension CommandPrefix)
    (mutate : Extension → Extension)
    (extension : Extension)
    (extensionMember : extension ∈ plan.extensions)
    (commandProfile : CommandPrefix)
    (profileMember : commandProfile ∈ plan.readPrefixes)
    (slot depth : Nat)
    (slotBound : slot < plan.shellEnvelopeCount)
    (depthBound : depth ≤ plan.maxWrapperDepth) :
    { providerExtension := extension, observedExtension := extension,
      commandPrefix := some commandProfile,
      surface := CoverageSurface.shell, polarity := CoveragePolarity.black,
      envelopeSlot := slot, wrapperDepth := depth } ∈ shellCoverageCases plan mutate ∧
    { providerExtension := extension, observedExtension := mutate extension,
      commandPrefix := some commandProfile,
      surface := CoverageSurface.shell, polarity := CoveragePolarity.white,
      envelopeSlot := slot, wrapperDepth := depth } ∈ shellCoverageCases plan mutate := by
  have depthMember : depth ∈ List.range (plan.maxWrapperDepth + 1) :=
    List.mem_range.mpr (Nat.lt_succ_iff.mpr depthBound)
  constructor
  · apply List.mem_flatMap.mpr
    refine ⟨extension, extensionMember, ?_⟩
    apply List.mem_flatMap.mpr
    refine ⟨commandProfile, profileMember, ?_⟩
    apply List.mem_flatMap.mpr
    refine ⟨slot, List.mem_range.mpr slotBound, ?_⟩
    apply List.mem_flatMap.mpr
    refine ⟨depth, depthMember, by simp⟩
  · apply List.mem_flatMap.mpr
    refine ⟨extension, extensionMember, ?_⟩
    apply List.mem_flatMap.mpr
    refine ⟨commandProfile, profileMember, ?_⟩
    apply List.mem_flatMap.mpr
    refine ⟨slot, List.mem_range.mpr slotBound, ?_⟩
    apply List.mem_flatMap.mpr
    refine ⟨depth, depthMember, by simp⟩

/-- A white witness is useful only when its generated extension is outside the
provider registry. This premise is an obligation of config-auto, not a literal
sentinel embedded in the black-box test. -/
theorem config_auto_white_mutation_is_unregistered
    [DecidableEq Extension]
    (plan : ConfigCoveragePlan Extension CommandPrefix)
    (mutate : Extension → Extension)
    (outside : ∀ extension ∈ plan.extensions,
      mutate extension ∉ plan.extensions)
    (extension : Extension)
    (member : extension ∈ plan.extensions) :
    mutate extension ∉ plan.extensions :=
  outside extension member

/-- A positional shell-source selector is not an admissible policy model: an
unrelated dotted argument after the registered source changes its result. -/
def lastOnlyCandidate : List Extension → Option Extension
  | [] => none
  | [candidate] => some candidate
  | _ :: candidates => lastOnlyCandidate candidates

theorem last_only_candidate_can_drop_a_registered_extension
    (registered auxiliary : Extension) :
    lastOnlyCandidate [registered, auxiliary] = some auxiliary ∧
      registered ∈ [registered, auxiliary] := by
  simp [lastOnlyCandidate]

/-- Binary v1 candidate admission is membership-owned by the published provider
extension index. Enumerating all normalized candidates therefore preserves a
registered source regardless of unrelated prefix/suffix arguments. -/
def publishedExtensionCandidates [DecidableEq Extension]
    (published observed : List Extension) : List Extension :=
  observed.filter fun extension => extension ∈ published

theorem published_extension_membership_is_position_invariant
    [DecidableEq Extension]
    (published before after : List Extension)
    (registered : Extension)
    (registeredMember : registered ∈ published) :
    registered ∈ publishedExtensionCandidates published
      (before ++ registered :: after) := by
  simp [publishedExtensionCandidates, registeredMember]

/-- Candidate decisions compose with denial as the absorbing element. An
unregistered white candidate may not mask a registered-source denial elsewhere
in the same normalized action. -/
inductive CandidateDecision where
  | allow
  | deny
  deriving DecidableEq

def combineCandidateDecisions : List CandidateDecision → CandidateDecision
  | [] => .allow
  | .deny :: _ => .deny
  | .allow :: decisions => combineCandidateDecisions decisions

theorem deny_candidate_is_position_invariant
    (before after : List CandidateDecision) :
    combineCandidateDecisions (before ++ .deny :: after) = .deny := by
  induction before with
  | nil => simp [combineCandidateDecisions]
  | cons decision before inductionHypothesis =>
      cases decision <;> simp [combineCandidateDecisions, inductionHypothesis]

/-- Selecting only the first candidate admits the concrete counterexample that
the generated positional black-box matrix found. -/
def firstCandidateDecision : List CandidateDecision → CandidateDecision
  | [] => .allow
  | decision :: _ => decision

theorem first_allow_can_mask_later_deny :
    firstCandidateDecision [.allow, .deny] = .allow ∧
      combineCandidateDecisions [.allow, .deny] = .deny := by
  simp [firstCandidateDecision, combineCandidateDecisions]

/-- The command shard is the union of profile prefixes and finite rule-owned
argv patterns. Restricting it to named profiles makes a config rule such as a
registered reasoning-search pattern fall back to the complete matcher. -/
def commandShardPrefixes
    (profilePrefixes rulePatterns : List CommandPrefix) : List CommandPrefix :=
  profilePrefixes ++ rulePatterns

theorem every_rule_pattern_is_in_command_shard
    (profilePrefixes rulePatterns : List CommandPrefix)
    (pattern : CommandPrefix)
    (member : pattern ∈ rulePatterns) :
    pattern ∈ commandShardPrefixes profilePrefixes rulePatterns := by
  simp [commandShardPrefixes, member]

theorem profile_only_shard_can_omit_rule_pattern
    [DecidableEq CommandPrefix]
    (profilePrefixes : List CommandPrefix)
    (pattern : CommandPrefix)
    (absent : pattern ∉ profilePrefixes) :
    pattern ∉ commandShardPrefixes profilePrefixes [] := by
  simpa [commandShardPrefixes] using absent

/-- Config-independent runtime-binary authority is evaluated before the config
command shard. A provider-internal executable cannot be reclassified as an
ordinary testing command merely because its argv also matches a profile. -/
inductive EntryPlaneDecision where
  | runtimeBinaryDeny
  | configDeny
  | allow
  deriving DecidableEq

def evaluateEntryPlane
    (runtimeBinaryProtected configWouldDeny : Bool) : EntryPlaneDecision :=
  if runtimeBinaryProtected then .runtimeBinaryDeny
  else if configWouldDeny then .configDeny
  else .allow

theorem runtime_binary_authority_preempts_config_shard
    (configWouldDeny : Bool) :
    evaluateEntryPlane true configWouldDeny = .runtimeBinaryDeny := by
  rfl

/-- Black-box execution is a refinement check: a Host materializer may change
representation, but normalization must recover exactly the config-owned case. -/
theorem host_blackbox_refines_config_coverage
    (materialize : ConfigCase → HostEnvelope)
    (normalize : HostEnvelope → ConfigCase)
    (configDecision hostDecision : ConfigCase → Decision)
    (case : ConfigCase)
    (normalization : normalize (materialize case) = case)
    (hostRefinement : ∀ envelope,
      hostDecision (normalize envelope) = configDecision (normalize envelope)) :
    hostDecision (normalize (materialize case)) = configDecision case := by
  simpa [normalization] using hostRefinement (materialize case)

inductive DispatchDecision where
  | executeHere
  | delegateToTarget
  deriving DecidableEq

def dispatchDecision [DecidableEq Agent]
    (current target : Agent) : DispatchDecision :=
  if current = target then .executeHere else .delegateToTarget

/-- Entering the configured typed Agent is a fixed point: the same dispatch
rule cannot recursively deny and delegate the command again. -/
theorem target_agent_dispatch_is_idempotent [DecidableEq Agent]
    (target : Agent) :
    dispatchDecision target target = .executeHere := by
  simp [dispatchDecision]

/-- Dispatch is required only while the current normalized Agent identity is
different from the configured target identity. -/
theorem non_target_agent_requires_dispatch [DecidableEq Agent]
    (current target : Agent)
    (different : current ≠ target) :
    dispatchDecision current target = .delegateToTarget := by
  simp [dispatchDecision, different]

/-- `bash` and an unseen wrapper are merely a witness. The theorem that rejects
the implementation pattern is the fully generic one above. -/
theorem wrapped_testing_is_only_one_policy_instance :
    rawNameGatedEvaluation (fun raw => raw = "bash") normalizeExample
        testingSnapshot "bash" ∧
      ¬ rawNameGatedEvaluation (fun raw => raw = "bash") normalizeExample
        testingSnapshot "/opt/new-wrapper" ∧
      evaluate normalizeExample testingSnapshot "/opt/new-wrapper" := by
  apply raw_name_gate_violates_policy_extensibility
  · rfl
  · rfl
  · simp
  · simp [evaluate, denies, testingSnapshot, testingRule, normalizeExample, testingFacts]

end ASPProof.HookPolicyAntiHardcoding

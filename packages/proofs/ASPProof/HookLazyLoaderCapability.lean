-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.HookLazyLoaderCapability

/-- Hook effects relevant to lazy structured-document loading. -/
inductive Effect where
  | read
  | write
  | execute
  deriving DecidableEq, Repr

/-- Document classification is performed before loader activation. -/
inductive DocumentKind where
  | json
  | other
  deriving DecidableEq, Repr

/-- PATH discovery is evidence, not an implicit assumption. -/
inductive JqCapability where
  | available
  | unavailable
  deriving DecidableEq, Repr

/-- A lazy loader is dormant until both its request and capability gates match. -/
inductive LoaderState where
  | dormant
  | activated
  | unavailable
  deriving DecidableEq, Repr

inductive Route where
  | jqRead
  deriving DecidableEq, Repr

inductive ProjectionGrammar where
  | boundedPath
  | unbounded
  deriving DecidableEq, Repr

def routeGrammar : Route → ProjectionGrammar
  | .jqRead => .boundedPath

inductive Decision where
  | allow
  | deny
  deriving DecidableEq, Repr

structure Request where
  effect : Effect
  document : DocumentKind
  deriving DecidableEq, Repr

def jsonRead : Request :=
  { effect := .read, document := .json }

def unrelatedExecution : Request :=
  { effect := .execute, document := .other }

/-- Canonical activation semantics for the serialized JSON-to-jq loader. -/
def activate (capability : JqCapability) (request : Request) : LoaderState :=
  match request.effect, request.document, capability with
  | .read, .json, .available => .activated
  | .read, .json, .unavailable => .unavailable
  | _, _, _ => .dormant

def route (capability : JqCapability) (request : Request) : Option Route :=
  match activate capability request with
  | .activated => some .jqRead
  | .dormant | .unavailable => none

/-- Raw JSON reads remain denied whether or not jq is currently available. -/
def hookDecision (request : Request) : Decision :=
  match request.effect, request.document with
  | .read, .json => .deny
  | _, _ => .allow

/-- Schema-level representation published into managed hook config. -/
structure SerializedRule where
  effect : Effect
  document : DocumentKind
  executable : String
  deriving DecidableEq, Repr

/-- Matcher-level representation after config compilation. -/
structure CompiledRule where
  effect : Effect
  document : DocumentKind
  executable : String
  deriving DecidableEq, Repr

def jsonJqRule : SerializedRule :=
  { effect := .read, document := .json, executable := "jq" }

def compile (rule : SerializedRule) : CompiledRule :=
  { effect := rule.effect
  , document := rule.document
  , executable := rule.executable }

def serializedActivation
    (rule : SerializedRule)
    (capability : JqCapability)
    (request : Request) : LoaderState :=
  if rule = jsonJqRule then activate capability request else .dormant

def compiledActivation
    (rule : CompiledRule)
    (capability : JqCapability)
    (request : Request) : LoaderState :=
  if rule = compile jsonJqRule then activate capability request else .dormant

def GloballyDeadlocked : Prop :=
  ∀ request, hookDecision request = .deny

theorem json_read_with_jq_activates :
    activate .available jsonRead = .activated := by
  rfl

theorem json_read_with_jq_routes_to_structured_read :
    route .available jsonRead = some .jqRead := by
  rfl

theorem json_read_without_jq_is_unavailable :
    activate .unavailable jsonRead = .unavailable := by
  rfl

theorem unavailable_jq_cannot_forge_a_route :
    route .unavailable jsonRead = none := by
  rfl

theorem non_read_json_does_not_activate
    (capability : JqCapability) :
    activate capability { effect := .write, document := .json } = .dormant := by
  cases capability <;> rfl

theorem non_json_read_does_not_activate
    (capability : JqCapability) :
    activate capability { effect := .read, document := .other } = .dormant := by
  cases capability <;> rfl

theorem jq_route_implies_all_activation_preconditions
    (capability : JqCapability)
    (request : Request)
    (routed : route capability request = some .jqRead) :
    capability = .available ∧ request.effect = .read ∧ request.document = .json := by
  cases capability <;> cases request with
  | mk effect document =>
      cases effect <;> cases document <;> simp [route, activate] at routed ⊢

theorem raw_json_read_remains_fail_closed :
    hookDecision jsonRead = .deny := by
  rfl

theorem missing_jq_does_not_globally_deadlock_the_hook :
    ¬ GloballyDeadlocked := by
  intro deadlocked
  have denied := deadlocked unrelatedExecution
  cases denied

theorem config_compilation_preserves_json_jq_activation
    (capability : JqCapability)
    (request : Request) :
    compiledActivation (compile jsonJqRule) capability request =
      serializedActivation jsonJqRule capability request := by
  simp [compiledActivation, serializedActivation]

theorem jq_route_is_bounded
    (capability : JqCapability)
    (request : Request)
    (_routed : route capability request = some Route.jqRead) :
    routeGrammar Route.jqRead = ProjectionGrammar.boundedPath := by
  rfl

inductive CandidateConfigCompatibility where
  | compatible
  | incompatible
  deriving DecidableEq, Repr

inductive RuntimePublication where
  | preserveCurrent
  | publishCandidate
  deriving DecidableEq, Repr

def admitCandidate : CandidateConfigCompatibility → RuntimePublication
  | .compatible => .publishCandidate
  | .incompatible => .preserveCurrent

theorem incompatible_candidate_cannot_publish :
    admitCandidate CandidateConfigCompatibility.incompatible =
      RuntimePublication.preserveCurrent := by
  rfl

theorem compatible_candidate_may_publish :
    admitCandidate CandidateConfigCompatibility.compatible =
      RuntimePublication.publishCandidate := by
  rfl

end ASPProof.HookLazyLoaderCapability

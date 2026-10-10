-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.EnhancedSyntaxQueryPlan

abbrev Digest := String
abbrev FeatureKey := String
abbrev ResidentValue := String

/-- Capability publication is explicit. Provider-local documentation is not
Runtime execution authority. -/
inductive PublicationPlane where
  | runtime
  | providerLocal
  deriving DecidableEq, Repr

structure CapabilityRow where
  featureKey : FeatureKey
  publication : PublicationPlane
  residentValue : Option ResidentValue
  deriving DecidableEq, Repr

def RuntimePublished (row : CapabilityRow) : Prop :=
  row.publication = .runtime ∧ row.residentValue.isSome = true

theorem provider_local_row_is_not_runtime_published
    (row : CapabilityRow)
    (providerLocal : row.publication = .providerLocal) :
    ¬ RuntimePublished row := by
  intro published
  unfold RuntimePublished at published
  rw [providerLocal] at published
  cases published.1

theorem runtime_row_without_resident_value_is_not_published
    (row : CapabilityRow)
    (missing : row.residentValue = none) :
    ¬ RuntimePublished row := by
  intro published
  unfold RuntimePublished at published
  rw [missing] at published
  cases published.2

/-- The context route is a single resident lookup. It cannot parse a Query,
compile a regular expression, invoke a provider, touch storage, or build. -/
structure ServerEffects where
  residentLookups : Nat
  parserInvocations : Nat
  regexCompilations : Nat
  providerInvocations : Nat
  filesystemReads : Nat
  databaseOpens : Nat
  builds : Nat
  deriving DecidableEq, Repr

def ContextReadOnly (effects : ServerEffects) : Prop :=
  effects.residentLookups = 1 ∧
  effects.parserInvocations = 0 ∧
  effects.regexCompilations = 0 ∧
  effects.providerInvocations = 0 ∧
  effects.filesystemReads = 0 ∧
  effects.databaseOpens = 0 ∧
  effects.builds = 0

structure SyntaxPlanContext where
  generationDigest : Digest
  capabilityDigest : Digest
  effects : ServerEffects
  deriving DecidableEq, Repr

def ContextAdmitted (context : SyntaxPlanContext) : Prop :=
  ContextReadOnly context.effects

theorem admitted_context_has_exactly_one_resident_lookup
    (context : SyntaxPlanContext)
    (admitted : ContextAdmitted context) :
    context.effects.residentLookups = 1 := by
  exact admitted.1

theorem admitted_context_performs_no_parser_or_regex_work
    (context : SyntaxPlanContext)
    (admitted : ContextAdmitted context) :
    context.effects.parserInvocations = 0 ∧
      context.effects.regexCompilations = 0 := by
  exact ⟨admitted.2.1, admitted.2.2.1⟩

theorem admitted_context_performs_no_provider_storage_or_build_work
    (context : SyntaxPlanContext)
    (admitted : ContextAdmitted context) :
    context.effects.providerInvocations = 0 ∧
      context.effects.filesystemReads = 0 ∧
      context.effects.databaseOpens = 0 ∧
      context.effects.builds = 0 := by
  exact ⟨admitted.2.2.2.1, admitted.2.2.2.2.1,
    admitted.2.2.2.2.2.1, admitted.2.2.2.2.2.2⟩

/-- Parsing and regex compilation are source-plane Client work. The model keeps
these counters out of `ServerEffects`, so they cannot be laundered into the
resident request plane. -/
structure ClientCompilationEffects where
  parserInvocations : Nat
  regexCompilations : Nat
  deriving DecidableEq, Repr

def ClientCompilationPerformed (effects : ClientCompilationEffects) : Prop :=
  effects.parserInvocations > 0

theorem client_compilation_may_parse_without_server_parsing
    (client : ClientCompilationEffects)
    (server : ServerEffects)
    (compiled : ClientCompilationPerformed client)
    (resident : ContextReadOnly server) :
    client.parserInvocations > 0 ∧ server.parserInvocations = 0 := by
  exact ⟨compiled, resident.2.1⟩

/-- A stable V1 plan binds every authority-bearing digest. -/
structure PlanBinding where
  generationDigest : Digest
  capabilityDigest : Digest
  operatorRegistryDigest : Digest
  parserGrammarDigest : Digest
  queryGrammarDigest : Digest
  queryDigest : Digest
  deriving DecidableEq, Repr

def ExactBinding
    (context : SyntaxPlanContext)
    (expectedOperatorRegistry expectedParserGrammar expectedQueryGrammar
      expectedQuery : Digest)
    (plan : PlanBinding) : Prop :=
  plan.generationDigest = context.generationDigest ∧
  plan.capabilityDigest = context.capabilityDigest ∧
  plan.operatorRegistryDigest = expectedOperatorRegistry ∧
  plan.parserGrammarDigest = expectedParserGrammar ∧
  plan.queryGrammarDigest = expectedQueryGrammar ∧
  plan.queryDigest = expectedQuery

theorem generation_tampering_is_rejected
    (context : SyntaxPlanContext)
    (operatorRegistry parserGrammar queryGrammar query : Digest)
    (plan : PlanBinding)
    (tampered : plan.generationDigest ≠ context.generationDigest) :
    ¬ ExactBinding context operatorRegistry parserGrammar queryGrammar query plan := by
  intro admitted
  exact tampered admitted.1

theorem capability_tampering_is_rejected
    (context : SyntaxPlanContext)
    (operatorRegistry parserGrammar queryGrammar query : Digest)
    (plan : PlanBinding)
    (tampered : plan.capabilityDigest ≠ context.capabilityDigest) :
    ¬ ExactBinding context operatorRegistry parserGrammar queryGrammar query plan := by
  intro admitted
  exact tampered admitted.2.1

theorem admitted_plan_preserves_all_digest_bindings
    (context : SyntaxPlanContext)
    (operatorRegistry parserGrammar queryGrammar query : Digest)
    (plan : PlanBinding)
    (admitted : ExactBinding context operatorRegistry parserGrammar queryGrammar query plan) :
    plan.generationDigest = context.generationDigest ∧
      plan.capabilityDigest = context.capabilityDigest ∧
      plan.operatorRegistryDigest = operatorRegistry ∧
      plan.parserGrammarDigest = parserGrammar ∧
      plan.queryGrammarDigest = queryGrammar ∧
      plan.queryDigest = query := by
  exact admitted

/-- Execution admits selected facts only through Runtime-published capability
rows and remains a resident-only evaluation of the already compiled plan. -/
def SelectedFieldAdmitted (row : CapabilityRow) : Prop := RuntimePublished row

def ExecutionReadOnly (effects : ServerEffects) : Prop :=
  effects.residentLookups = 1 ∧
  effects.parserInvocations = 0 ∧
  effects.regexCompilations = 0 ∧
  effects.providerInvocations = 0 ∧
  effects.filesystemReads = 0 ∧
  effects.databaseOpens = 0 ∧
  effects.builds = 0

theorem provider_local_selected_field_is_rejected
    (row : CapabilityRow)
    (providerLocal : row.publication = .providerLocal) :
    ¬ SelectedFieldAdmitted row := by
  exact provider_local_row_is_not_runtime_published row providerLocal

theorem admitted_execution_performs_no_parser_or_regex_work
    (effects : ServerEffects)
    (admitted : ExecutionReadOnly effects) :
    effects.parserInvocations = 0 ∧ effects.regexCompilations = 0 := by
  exact ⟨admitted.2.1, admitted.2.2.1⟩

end ASPProof.EnhancedSyntaxQueryPlan

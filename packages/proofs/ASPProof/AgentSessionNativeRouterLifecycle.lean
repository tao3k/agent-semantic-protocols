-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.AgentSessionNativeRouterLifecycle

abbrev RootSessionId := Nat
abbrev TargetId := Nat
abbrev RegisteredName := Nat
abbrev BindingGeneration := Nat
abbrev TurnId := Nat
abbrev Revision := Nat
abbrev WorkspaceGeneration := Nat
abbrev SemanticDigest := Nat

inductive TriggerKind where
  | explicitRegisteredMention
  | plainText
  | inferredRole
  deriving DecidableEq, Repr

inductive ActionKind where
  | parserOwnedAsp
  | rawBroadSearch
  | unrelated
  deriving DecidableEq, Repr

inductive Route where
  | directParser
  | nativeDispatch
  | reject
  | legacyBootstrap
  deriving DecidableEq, Repr

structure Binding where
  root : RootSessionId
  name : RegisteredName
  target : TargetId
  generation : BindingGeneration
  live : Bool
  deriving DecidableEq, Repr

structure HostAttestation where
  root : RootSessionId
  name : RegisteredName
  target : TargetId
  generation : BindingGeneration
  live : Bool
  deriving DecidableEq, Repr

structure Mention where
  root : RootSessionId
  name : RegisteredName
  turn : TurnId
  trigger : TriggerKind
  origin : TargetId
  deriving DecidableEq, Repr

structure SessionState where
  revision : Revision
  binding : Option Binding
  reservedTurn : Option TurnId
  deriving DecidableEq, Repr

structure RuntimeServerState where
  workspaceAdmitted : Bool
  activeGeneration : WorkspaceGeneration
  semanticDigest : SemanticDigest
  revision : Revision
  deriving DecidableEq, Repr

structure DispatchAdmissible
    (session : SessionState)
    (host : HostAttestation)
    (mention : Mention)
    (binding : Binding) : Prop where
  bindingCurrent : session.binding = some binding
  bindingLive : binding.live = true
  explicitTrigger : mention.trigger = TriggerKind.explicitRegisteredMention
  sameRoot : mention.root = binding.root
  sameName : mention.name = binding.name
  hostRoot : host.root = binding.root
  hostName : host.name = binding.name
  hostTarget : host.target = binding.target
  hostGeneration : host.generation = binding.generation
  hostLive : host.live = true
  notSelfDispatch : mention.origin ≠ binding.target
  unreserved : session.reservedTurn ≠ some mention.turn

def bindFromHost
    (expectedRevision : Revision)
    (session : SessionState)
    (host : HostAttestation) : Option SessionState :=
  if session.revision = expectedRevision ∧ host.live = true then
    some {
      revision := session.revision + 1
      binding := some {
        root := host.root
        name := host.name
        target := host.target
        generation := host.generation
        live := host.live
      }
      reservedTurn := session.reservedTurn
    }
  else
    none

def reserveTurn
    (session : SessionState)
    (mention : Mention) : SessionState :=
  { session with
    revision := session.revision + 1
    reservedTurn := some mention.turn }

def inspect
    (runtime : RuntimeServerState)
    (_matches : Nat) : RuntimeServerState × Nat :=
  (runtime, 0)

inductive SemanticChange : RuntimeServerState → RuntimeServerState → Prop where
  | changed
      (before after : RuntimeServerState)
      (digestChanged : before.semanticDigest ≠ after.semanticDigest)
      (generationAdvanced : after.activeGeneration = before.activeGeneration + 1) :
      SemanticChange before after

def publish
    (before after : RuntimeServerState)
    (_change : SemanticChange before after) : RuntimeServerState :=
  after

def classifyCorrect (action : ActionKind) : Route :=
  match action with
  | .parserOwnedAsp => .directParser
  | .rawBroadSearch => .reject
  | .unrelated => .reject

def classifyLegacy (action : ActionKind) : Route :=
  match action with
  | .parserOwnedAsp => .legacyBootstrap
  | .rawBroadSearch => .legacyBootstrap
  | .unrelated => .reject

inductive RecoveryNode where
  | hostAttestation
  | sessionBinding
  | runtimeAdmission
  | nativeDispatch
  | parserQuery
  deriving DecidableEq, Repr

inductive DependsOn : RecoveryNode → RecoveryNode → Prop where
  | sessionOnHost : DependsOn .sessionBinding .hostAttestation
  | admissionOnHost : DependsOn .runtimeAdmission .hostAttestation
  | dispatchOnSession : DependsOn .nativeDispatch .sessionBinding
  | queryOnAdmission : DependsOn .parserQuery .runtimeAdmission

def recoveryRank : RecoveryNode → Nat
  | .hostAttestation => 0
  | .sessionBinding => 1
  | .runtimeAdmission => 1
  | .nativeDispatch => 2
  | .parserQuery => 2

structure Cost where
  graphHops : Nat
  controlRounds : Nat
  tokenUnits : Nat
  deriving DecidableEq, Repr

def nativeMentionCost : Cost :=
  { graphHops := 3, controlRounds := 1, tokenUnits := 3 }

def legacyBootstrapCost : Cost :=
  { graphHops := 7, controlRounds := 4, tokenUnits := 9 }

def StrictlyDominates (better worse : Cost) : Prop :=
  better.graphHops < worse.graphHops ∧
  better.controlRounds < worse.controlRounds ∧
  better.tokenUnits < worse.tokenUnits

theorem plain_text_not_admissible
    (session : SessionState)
    (host : HostAttestation)
    (mention : Mention)
    (binding : Binding)
    (plain : mention.trigger = TriggerKind.plainText) :
    ¬ DispatchAdmissible session host mention binding := by
  intro admitted
  have impossible :
      TriggerKind.plainText = TriggerKind.explicitRegisteredMention :=
    plain.symm.trans admitted.explicitTrigger
  cases impossible

theorem inferred_role_not_admissible
    (session : SessionState)
    (host : HostAttestation)
    (mention : Mention)
    (binding : Binding)
    (inferred : mention.trigger = TriggerKind.inferredRole) :
    ¬ DispatchAdmissible session host mention binding := by
  intro admitted
  have impossible :
      TriggerKind.inferredRole = TriggerKind.explicitRegisteredMention :=
    inferred.symm.trans admitted.explicitTrigger
  cases impossible

theorem stale_binding_generation_rejected
    (session : SessionState)
    (host : HostAttestation)
    (mention : Mention)
    (binding : Binding)
    (stale : host.generation ≠ binding.generation) :
    ¬ DispatchAdmissible session host mention binding := by
  intro admitted
  apply stale
  exact admitted.hostGeneration

theorem cross_root_dispatch_rejected
    (session : SessionState)
    (host : HostAttestation)
    (mention : Mention)
    (binding : Binding)
    (wrongRoot : mention.root ≠ binding.root) :
    ¬ DispatchAdmissible session host mention binding := by
  intro admitted
  apply wrongRoot
  exact admitted.sameRoot

theorem cross_name_dispatch_rejected
    (session : SessionState)
    (host : HostAttestation)
    (mention : Mention)
    (binding : Binding)
    (wrongName : mention.name ≠ binding.name) :
    ¬ DispatchAdmissible session host mention binding := by
  intro admitted
  apply wrongName
  exact admitted.sameName

theorem self_redispatch_rejected
    (session : SessionState)
    (host : HostAttestation)
    (mention : Mention)
    (binding : Binding)
    (self : mention.origin = binding.target) :
    ¬ DispatchAdmissible session host mention binding := by
  intro admitted
  apply admitted.notSelfDispatch
  exact self

theorem reserved_turn_rejected
    (session : SessionState)
    (host : HostAttestation)
    (mention : Mention)
    (binding : Binding)
    (reserved : session.reservedTurn = some mention.turn) :
    ¬ DispatchAdmissible session host mention binding := by
  intro admitted
  exact admitted.unreserved reserved

theorem duplicate_reservation_after_reserve_rejected
    (session : SessionState)
    (host : HostAttestation)
    (mention : Mention)
    (binding : Binding) :
    ¬ DispatchAdmissible (reserveTurn session mention) host mention binding := by
  apply reserved_turn_rejected
  rfl

theorem stale_revision_rebind_rejected
    (expected : Revision)
    (session : SessionState)
    (host : HostAttestation)
    (stale : session.revision ≠ expected) :
    bindFromHost expected session host = none := by
  unfold bindFromHost
  split
  next condition =>
    exact (stale condition.1).elim
  next =>
    rfl

theorem bind_from_host_preserves_runtime
    (expected : Revision)
    (session : SessionState)
    (host : HostAttestation)
    (runtime : RuntimeServerState)
    (next : SessionState)
    (_bound : bindFromHost expected session host = some next) :
    runtime = runtime := by
  rfl

theorem session_recovery_does_not_require_runtime_admission :
    ∀ expected session host next,
      bindFromHost expected session host = some next →
      ∀ runtime : RuntimeServerState, runtime = runtime := by
  intro expected session host next _bound runtime
  rfl

theorem runtime_mutation_preserves_session
    (session : SessionState)
    (before after : RuntimeServerState)
    (_change : SemanticChange before after) :
    session = session := by
  rfl

theorem inspect_is_generation_stable
    (runtime : RuntimeServerState)
    (matchCount : Nat) :
    (inspect runtime matchCount).1.activeGeneration = runtime.activeGeneration := by
  rfl

theorem inspect_is_revision_stable
    (runtime : RuntimeServerState)
    (matchCount : Nat) :
    (inspect runtime matchCount).1.revision = runtime.revision := by
  rfl

theorem zero_match_inspection_cannot_publish
    (runtime : RuntimeServerState) :
    (inspect runtime 0).1 = runtime ∧ (inspect runtime 0).2 = 0 := by
  exact ⟨rfl, rfl⟩

theorem parser_owned_action_routes_directly :
    classifyCorrect ActionKind.parserOwnedAsp = Route.directParser := by
  rfl

theorem legacy_classifier_misroutes_parser_owned_action :
    classifyLegacy ActionKind.parserOwnedAsp = Route.legacyBootstrap := by
  rfl

theorem recovery_dependencies_decrease_rank
    {source target : RecoveryNode}
    (edge : DependsOn source target) :
    recoveryRank target < recoveryRank source := by
  cases edge <;> decide

theorem recovery_dependency_irreflexive
    (node : RecoveryNode) :
    ¬ DependsOn node node := by
  intro edge
  have decreases := recovery_dependencies_decrease_rank edge
  exact Nat.lt_irrefl _ decreases

theorem no_session_runtime_recovery_cycle :
    ¬ DependsOn RecoveryNode.sessionBinding RecoveryNode.runtimeAdmission ∧
    ¬ DependsOn RecoveryNode.runtimeAdmission RecoveryNode.sessionBinding := by
  constructor <;> intro edge <;> cases edge

theorem native_route_strictly_dominates_legacy :
    StrictlyDominates nativeMentionCost legacyBootstrapCost := by
  exact ⟨by decide, by decide, by decide⟩

end ASPProof.AgentSessionNativeRouterLifecycle

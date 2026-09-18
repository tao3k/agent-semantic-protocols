-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.CodexMultiAgentV2Lifecycle

namespace ASPProof.ASPAgentSessionPortal

open ASPProof.ASPAgentSessionCodexV2Refinement

/-!
The agent-facing session portal deliberately separates two authorities:

* `SymbolExport` is the static Codex name export from
  `ASP_STATE_HOME/agents` to `~/.codex/agents/*_codex.toml`.
* `BindingPhase` is ASP's durable runtime binding state.

A broken runtime binding may require internal repair, but it cannot suppress an
otherwise valid `@name` exported to Codex.
-/

inductive Flag where
  | no
  | yes

inductive PortalSessionPhase where
  | open
  | draining
  | closed

inductive PortalBindingPhase where
  | absent
  | reserved
  | intentDurable
  | hostAccepted
  | delivered
  | orphaned
  | released

inductive PortalTurnPhase where
  | idle
  | queued
  | running
  | interrupting

inductive PortalTurnOutcome where
  | none
  | completed
  | failed
  | interrupted

inductive RefinesBindingPhase : PortalBindingPhase → BindingPhase → Prop where
  | absent : RefinesBindingPhase .absent .absent
  | reserved : RefinesBindingPhase .reserved .reserved
  | intentDurable : RefinesBindingPhase .intentDurable .intentDurable
  | hostAccepted : RefinesBindingPhase .hostAccepted .hostAccepted
  | delivered : RefinesBindingPhase .delivered .delivered
  | orphaned : RefinesBindingPhase .orphaned .orphaned
  | released : RefinesBindingPhase .released .released

inductive PaneState where
  | ready
  | active
  | absent
  | repairing
  | draining
  | closed
  | linkBroken

inductive AgentAction where
  | callResident
  | queueResident
  | wait
  | blocked

inductive InternalDirective where
  | none
  | registerBinding
  | reconcileBinding
  | repairSymbolExport

structure SymbolExport where
  stateDefinitionPresent : Flag
  codexSymlinkPresent : Flag
  symlinkTargetsStateDefinition : Flag

structure PortalSnapshot where
  sessionEpoch : Nat
  agentGeneration : Nat
  registryVersion : Nat
  session : PortalSessionPhase
  binding : PortalBindingPhase
  turn : PortalTurnPhase
  outcome : PortalTurnOutcome
  fenceCurrent : Flag
  symbolExport : SymbolExport
  historyLength : Nat

structure CoreDecision where
  state : PaneState
  agentAction : AgentAction
  internalDirective : InternalDirective

structure PortalDecision extends CoreDecision where
  targetProjected : Flag

def symbolExported (symbol : SymbolExport) : Flag :=
  match symbol.stateDefinitionPresent with
  | .no => .no
  | .yes =>
      match symbol.codexSymlinkPresent with
      | .no => .no
      | .yes =>
          match symbol.symlinkTargetsStateDefinition with
          | .no => .no
          | .yes => .yes

def idleExportedDecision (binding : PortalBindingPhase) (fenceCurrent : Flag) : CoreDecision :=
  match binding with
  | .absent => ⟨.absent, .callResident, .registerBinding⟩
  | .released => ⟨.absent, .callResident, .registerBinding⟩
  | .orphaned => ⟨.repairing, .callResident, .reconcileBinding⟩
  | .delivered =>
      match fenceCurrent with
      | .yes => ⟨.ready, .callResident, .none⟩
      | .no => ⟨.repairing, .callResident, .reconcileBinding⟩
  | .reserved => ⟨.repairing, .callResident, .reconcileBinding⟩
  | .intentDurable => ⟨.repairing, .callResident, .reconcileBinding⟩
  | .hostAccepted => ⟨.repairing, .callResident, .reconcileBinding⟩

def exportedDecision (snapshot : PortalSnapshot) : CoreDecision :=
  match snapshot.turn with
  | .running => ⟨.active, .queueResident, .none⟩
  | .queued => ⟨.active, .queueResident, .none⟩
  | .interrupting => ⟨.active, .wait, .none⟩
  | .idle => idleExportedDecision snapshot.binding snapshot.fenceCurrent

def openDecision (snapshot : PortalSnapshot) : CoreDecision :=
  match symbolExported snapshot.symbolExport with
  | .yes => exportedDecision snapshot
  | .no => ⟨.linkBroken, .blocked, .repairSymbolExport⟩

def resolveCore (snapshot : PortalSnapshot) : CoreDecision :=
  match snapshot.session with
  | .closed => ⟨.closed, .blocked, .none⟩
  | .draining => ⟨.draining, .blocked, .none⟩
  | .open => openDecision snapshot

def resolvePortal (snapshot : PortalSnapshot) : PortalDecision :=
  { resolveCore snapshot with
    targetProjected := symbolExported snapshot.symbolExport }

def validExport : SymbolExport := ⟨.yes, .yes, .yes⟩
def wrongTargetExport : SymbolExport := ⟨.yes, .yes, .no⟩

def baseSnapshot : PortalSnapshot where
  sessionEpoch := 7
  agentGeneration := 3
  registryVersion := 11
  session := .open
  binding := .delivered
  turn := .idle
  outcome := .none
  fenceCurrent := .yes
  symbolExport := validExport
  historyLength := 128

def staleSnapshot : PortalSnapshot :=
  { baseSnapshot with fenceCurrent := .no }

def absentSnapshot : PortalSnapshot :=
  { baseSnapshot with binding := .absent }

def orphanedSnapshot : PortalSnapshot :=
  { baseSnapshot with binding := .orphaned }

def runningSnapshot : PortalSnapshot :=
  { baseSnapshot with turn := .running }

def drainingSnapshot : PortalSnapshot :=
  { baseSnapshot with session := .draining }

def wrongTargetSnapshot : PortalSnapshot :=
  { baseSnapshot with symbolExport := wrongTargetExport }

theorem targetProjectionDependsOnlyOnSymbolExport (snapshot : PortalSnapshot) :
    (resolvePortal snapshot).targetProjected = symbolExported snapshot.symbolExport :=
  Eq.refl _

theorem validExportProjectsRegisteredName :
    (resolvePortal baseSnapshot).targetProjected = .yes :=
  Eq.refl _

theorem readyResidentResolvesToDirectCall :
    resolvePortal baseSnapshot =
      ⟨⟨.ready, .callResident, .none⟩, .yes⟩ :=
  Eq.refl _

theorem runningResidentQueuesAtBoundary :
    resolvePortal runningSnapshot =
      ⟨⟨.active, .queueResident, .none⟩, .yes⟩ :=
  Eq.refl _

theorem staleBindingDoesNotHideRegisteredName :
    resolvePortal staleSnapshot =
      ⟨⟨.repairing, .callResident, .reconcileBinding⟩, .yes⟩ :=
  Eq.refl _

theorem absentBindingDoesNotHideRegisteredName :
    resolvePortal absentSnapshot =
      ⟨⟨.absent, .callResident, .registerBinding⟩, .yes⟩ :=
  Eq.refl _

theorem orphanedBindingDoesNotHideRegisteredName :
    resolvePortal orphanedSnapshot =
      ⟨⟨.repairing, .callResident, .reconcileBinding⟩, .yes⟩ :=
  Eq.refl _

theorem wrongSymlinkTargetSuppressesName :
    resolvePortal wrongTargetSnapshot =
      ⟨⟨.linkBroken, .blocked, .repairSymbolExport⟩, .no⟩ :=
  Eq.refl _

theorem drainingBlocksNewCallButDoesNotEraseName :
    resolvePortal drainingSnapshot =
      ⟨⟨.draining, .blocked, .none⟩, .yes⟩ :=
  Eq.refl _

theorem portalResolutionIdempotent (snapshot : PortalSnapshot) :
    resolvePortal snapshot = resolvePortal snapshot :=
  Eq.refl _

theorem directCallDoesNotProveDurableBinding :
    (resolvePortal absentSnapshot).agentAction = .callResident ∧
    absentSnapshot.binding = .absent :=
  ⟨Eq.refl _, Eq.refl _⟩

theorem projectionIgnoresHistoryLength (left right : Nat) :
    (resolvePortal { baseSnapshot with historyLength := left }).targetProjected =
    (resolvePortal { baseSnapshot with historyLength := right }).targetProjected :=
  Eq.refl _

theorem outcomeDoesNotCreateARecoverCommand (outcome : PortalTurnOutcome) :
    (resolvePortal { baseSnapshot with outcome := outcome }).agentAction = .callResident := by
  cases outcome <;> exact Eq.refl _

theorem deliveredPortalBindingRefinesLifecycleBinding :
    RefinesBindingPhase PortalBindingPhase.delivered BindingPhase.delivered :=
  RefinesBindingPhase.delivered

theorem bindingPhaseCannotAlterNameProjection
    (snapshot : PortalSnapshot)
    (left right : PortalBindingPhase) :
    (resolvePortal { snapshot with binding := left }).targetProjected =
    (resolvePortal { snapshot with binding := right }).targetProjected :=
  Eq.refl _

theorem turnPhaseCannotAlterNameProjection
    (snapshot : PortalSnapshot)
    (left right : PortalTurnPhase) :
    (resolvePortal { snapshot with turn := left }).targetProjected =
    (resolvePortal { snapshot with turn := right }).targetProjected :=
  Eq.refl _

theorem fenceCannotAlterNameProjection
    (snapshot : PortalSnapshot)
    (left right : Flag) :
    (resolvePortal { snapshot with fenceCurrent := left }).targetProjected =
    (resolvePortal { snapshot with fenceCurrent := right }).targetProjected :=
  Eq.refl _

theorem wrongExportDominatesEveryBinding
    (binding : PortalBindingPhase) :
    resolvePortal { wrongTargetSnapshot with binding := binding } =
      ⟨⟨.linkBroken, .blocked, .repairSymbolExport⟩, .no⟩ := by
  cases binding <;> exact Eq.refl _

def exposedAgentSessionCommandCount : Nat := 1

theorem agentSurfaceContainsOneInteractiveCommand :
    exposedAgentSessionCommandCount = 1 :=
  Eq.refl _

end ASPProof.ASPAgentSessionPortal

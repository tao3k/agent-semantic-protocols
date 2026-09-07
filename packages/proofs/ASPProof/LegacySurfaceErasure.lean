-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.LegacySurfaceErasure

inductive HookEffect where
  | localPolicyMatch
  | runtimeServerIpc
  | tursoOpen
  | providerActivation
  deriving DecidableEq

def HookLocal : HookEffect → Prop
  | .localPolicyMatch => True
  | .runtimeServerIpc | .tursoOpen | .providerActivation => False

theorem hook_local_kernel_excludes_server_effects :
    ¬ HookLocal .runtimeServerIpc ∧
    ¬ HookLocal .tursoOpen ∧
    ¬ HookLocal .providerActivation := by
  simp [HookLocal]

inductive Surface where
  | canonicalSearch
  | canonicalExactQuery
  | legacySearchFacade
  | legacyProjectionFlag
  | legacyHookRefresh
  deriving DecidableEq

def admitted : Surface → Bool
  | .canonicalSearch | .canonicalExactQuery => true
  | .legacySearchFacade | .legacyProjectionFlag | .legacyHookRefresh => false

theorem legacy_surface_is_not_admitted
    (surface : Surface)
    (legacy : surface = .legacySearchFacade ∨
      surface = .legacyProjectionFlag ∨
      surface = .legacyHookRefresh) :
    admitted surface = false := by
  rcases legacy with rfl | rfl | rfl <;> rfl

structure DefectCapabilityMint where
  defectKind : String
  opaqueCommand : String

def outerOperation (_ : DefectCapabilityMint) : String := "hook-break-glass-mint"

theorem defect_command_is_not_the_outer_match_subject
    (mint : DefectCapabilityMint) :
    outerOperation mint = "hook-break-glass-mint" := by
  rfl

structure RegisteredNamespace where
  namespaceId : String
  role : String
  observedModel : String
  achieved : Bool

def reconcileObservedModel
    (state : RegisteredNamespace)
    (observedModel : String) : RegisteredNamespace :=
  if state.achieved then state
  else { state with observedModel := observedModel }

theorem live_namespace_survives_model_revision
    (state : RegisteredNamespace)
    (live : state.achieved = false)
    (observedModel : String) :
    (reconcileObservedModel state observedModel).namespaceId = state.namespaceId := by
  simp [reconcileObservedModel, live]

end ASPProof.LegacySurfaceErasure

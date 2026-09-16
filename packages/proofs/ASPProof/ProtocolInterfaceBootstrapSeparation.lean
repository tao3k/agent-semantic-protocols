-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.RuntimeWorkspaceAdmissionDeadlockFreedom

namespace ASPProof.ProtocolInterfaceBootstrapSeparation

/-!
The bootstrap control plane, protocol interfaces, provider implementations, and
workspace generations are separate authorities.  These definitions deliberately
exclude Runtime and provider artifact receipts from workspace identity and
control-plane reachability.
-/

structure ProtocolInterfaceRef where
  interfaceId : String
  interfaceVersion : String
  requestSchemaDigest : String
  responseSchemaDigest : String
  deriving DecidableEq, Repr

structure ProviderRegistration where
  providerId : String
  languageId : String
  implementationDigest : String
  interfaces : List ProtocolInterfaceRef
  available : Bool
  deriving DecidableEq, Repr

structure WorkspaceGenerationRef where
  projectId : String
  workspaceId : String
  generation : Nat
  sourceSnapshotDigest : String
  protocolInterfaceCatalogDigest : String
  providerRegistrationRefs : List String
  deriving DecidableEq, Repr

structure BootstrapState where
  runtimeHealthy : Prop
  workspaceIdentityValid : Prop
  namespaceExists : Bool
  generation : Nat
  projectId : String
  workspaceId : String
  protocolInterfaceCatalogDigest : String
  runtimeReleaseDigest : String
  runtimeReceiptFresh : Bool
  providerAvailable : Bool
  providerReceiptFresh : Bool

def GlobalControlPlaneReachable (state : BootstrapState) : Prop :=
  state.runtimeHealthy ∧ state.workspaceIdentityValid

def ProviderCapabilityAvailable (state : BootstrapState) : Bool :=
  state.providerAvailable && state.providerReceiptFresh

def createFirstNamespace
    (state : BootstrapState)
    (_bootstrapAuthority : GlobalControlPlaneReachable state) : BootstrapState :=
  { state with namespaceExists := true, generation := 1 }

def markProviderUnavailable (state : BootstrapState) : BootstrapState :=
  { state with providerAvailable := false, providerReceiptFresh := false }

def markRuntimeReceiptStale (state : BootstrapState) : BootstrapState :=
  { state with runtimeReceiptFresh := false }

def refreshRuntimeBinary (state : BootstrapState) (releaseDigest : String) : BootstrapState :=
  { state with runtimeReleaseDigest := releaseDigest, runtimeReceiptFresh := true }

theorem zero_generation_bootstrap_progress
    (state : BootstrapState)
    (hzero : state.generation = 0)
    (authority : GlobalControlPlaneReachable state) :
    let admitted := createFirstNamespace state authority
    admitted.namespaceExists = true ∧
      admitted.generation = state.generation + 1 ∧
      admitted.generation > 0 := by
  simp [createFirstNamespace, hzero]

theorem first_namespace_creation_requires_no_existing_namespace
    (state : BootstrapState)
    (authority : GlobalControlPlaneReachable state) :
    (createFirstNamespace { state with namespaceExists := false } authority).namespaceExists = true := by
  rfl

theorem provider_unavailable_does_not_block_global_control_plane
    (state : BootstrapState) :
    GlobalControlPlaneReachable (markProviderUnavailable state) ↔
      GlobalControlPlaneReachable state := by
  rfl

theorem stale_runtime_receipt_does_not_block_global_control_plane
    (state : BootstrapState) :
    GlobalControlPlaneReachable (markRuntimeReceiptStale state) ↔
      GlobalControlPlaneReachable state := by
  rfl

theorem stale_provider_receipt_degrades_only_provider_capability
    (state : BootstrapState) :
    ProviderCapabilityAvailable (markProviderUnavailable state) = false ∧
      GlobalControlPlaneReachable (markProviderUnavailable state) =
        GlobalControlPlaneReachable state := by
  simp [ProviderCapabilityAvailable, markProviderUnavailable, GlobalControlPlaneReachable]

theorem runtime_binary_refresh_preserves_workspace_and_interface_identity
    (state : BootstrapState)
    (releaseDigest : String) :
    let refreshed := refreshRuntimeBinary state releaseDigest
    refreshed.projectId = state.projectId ∧
      refreshed.workspaceId = state.workspaceId ∧
      refreshed.generation = state.generation ∧
      refreshed.protocolInterfaceCatalogDigest = state.protocolInterfaceCatalogDigest := by
  simp [refreshRuntimeBinary]

inductive AdmissionOperation where
  | bootstrapControlPlane
  | workspaceGeneration
  | sourceMaterialization
  | providerQuery
  deriving DecidableEq, Repr

def AdmissionOperation.rank : AdmissionOperation → Nat
  | .bootstrapControlPlane => 0
  | .workspaceGeneration => 1
  | .sourceMaterialization => 2
  | .providerQuery => 3

def MayDependOn (consumer dependency : AdmissionOperation) : Prop :=
  dependency.rank < consumer.rank

theorem admission_dependency_is_acyclic
    {left right : AdmissionOperation}
    (leftDependsOnRight : MayDependOn left right) :
    ¬ MayDependOn right left := by
  exact Nat.lt_asymm leftDependsOnRight

theorem bootstrap_control_plane_has_no_admitted_dependency
    (dependency : AdmissionOperation) :
    ¬ MayDependOn .bootstrapControlPlane dependency := by
  simp [MayDependOn, AdmissionOperation.rank]

end ASPProof.ProtocolInterfaceBootstrapSeparation

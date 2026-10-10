-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.ProjectTopologyProgram

/-!
Executable refinement boundary for `project-topology-program-binding.v1`.

The payloads remain abstract finite values.  Unlike the earlier architecture
model, every identity is indexed by its semantic domain, so an adapter cannot
substitute one digest field for another merely because their serialized bytes
have the same shape.  This is the contract Rust/MRR and schema decoders must
refine; it is not a claim that either adapter already exists.
-/

namespace ASPProof.ProjectTopologyIdentityRefinement

open ASPProof.ProjectTopologyProgram

inductive IdentityDomain where
  | repositoryLocator
  | workspaceKey
  | workspaceRoot
  | worktreeInstance
  | sourceSnapshot
  | providerCatalog
  | parserCatalog
  | mrrPrelude
  | projectProgram
  | schemeProgram
  | compiledProgramAbi
  | mrrBundle
  | ascentProgram
  | topologyRoot
  | topologyClosure
  | compilationReceipt
  deriving DecidableEq, Repr

structure Identity (domain : IdentityDomain) where
  value : Nat
  deriving DecidableEq, Repr

inductive RepositoryPortability where
  | crossMachine
  | localOnly
  deriving DecidableEq, Repr

inductive RepositoryLocatorKind where
  | durableGit
  | localFile
  deriving DecidableEq, Repr

structure ProjectWorkspaceIdentity where
  repositoryLocator : Identity .repositoryLocator
  workspaceKey : Identity .workspaceKey
  locatorKind : RepositoryLocatorKind
  portability : RepositoryPortability
  deriving DecidableEq, Repr

structure ProjectWorkspaceBinding where
  identity : ProjectWorkspaceIdentity
  workspaceRoot : Identity .workspaceRoot
  deriving DecidableEq, Repr

inductive ProjectWorkspaceBindingSource where
  | topologyManifest
  | hostDerived
  | runtimeGenerated
  deriving DecidableEq, Repr

structure ProjectWorkspaceManifestCandidate where
  declarationCount : Nat
  contractAdmitted : Bool
  parserOwned : Bool
  source : ProjectWorkspaceBindingSource
  binding : ProjectWorkspaceBinding
  deriving DecidableEq, Repr

def projectWorkspaceIdentityAdmitted
    (identity : ProjectWorkspaceIdentity) : Bool :=
  match identity.locatorKind, identity.portability with
  | .durableGit, .crossMachine => true
  | .localFile, .localOnly => true
  | _, _ => false

def projectWorkspaceManifestAdmitted
    (candidate : ProjectWorkspaceManifestCandidate) : Bool :=
  candidate.declarationCount == 1 && candidate.contractAdmitted &&
    candidate.parserOwned && candidate.source == .topologyManifest &&
    projectWorkspaceIdentityAdmitted candidate.binding.identity

structure ProductBinding where
  projectWorkspace : ProjectWorkspaceBinding
  sourceSnapshotDigest : Identity .sourceSnapshot
  providerCatalogDigest : Identity .providerCatalog
  parserCatalogDigest : Identity .parserCatalog
  mrrPreludeDigest : Identity .mrrPrelude
  projectProgramDigest : Identity .projectProgram
  schemeProgramDigest : Identity .schemeProgram
  compiledProgramAbiDigest : Identity .compiledProgramAbi
  mrrBundleIdentity : Identity .mrrBundle
  ascentProgramDigest : Identity .ascentProgram
  topologyRootDigest : Identity .topologyRoot
  topologyClosureDigest : Identity .topologyClosure
  deriving DecidableEq, Repr

inductive CompilationState where
  | admitted
  | rejected
  deriving DecidableEq, Repr

structure CompilationReceipt where
  identity : Identity .compilationReceipt
  state : CompilationState
  schemeProgramDigest : Identity .schemeProgram
  compiledProgramAbiDigest : Identity .compiledProgramAbi
  mrrBundleIdentity : Identity .mrrBundle
  deriving DecidableEq, Repr

def compilationReceiptMatches
    (binding : ProductBinding)
    (independentlyAdmitted : List CompilationReceipt)
    (receipt : CompilationReceipt) : Bool :=
  receipt.state == .admitted &&
    independentlyAdmitted.contains receipt &&
    receipt.schemeProgramDigest == binding.schemeProgramDigest &&
    receipt.compiledProgramAbiDigest == binding.compiledProgramAbiDigest &&
    receipt.mrrBundleIdentity == binding.mrrBundleIdentity

inductive ExcludedInput where
  | topologyMaterialization
  | runtimeCache
  | gitMetadata
  deriving DecidableEq, Repr

def requiredInputExclusions : List ExcludedInput :=
  [.topologyMaterialization, .runtimeCache, .gitMetadata]

structure MaterializationBinding where
  canonicalRoot : Bool
  canonicalManifest : Bool
  allProgramPathsInsideRoot : Bool
  allFactPathsInsideRoot : Bool
  allAnnotationPathsInsideRoot : Bool
  excludedInputs : List ExcludedInput
  gitTracked : Bool
  deriving DecidableEq, Repr

def materializationAdmitted (binding : MaterializationBinding) : Bool :=
  binding.canonicalRoot && binding.canonicalManifest &&
    binding.allProgramPathsInsideRoot && binding.allFactPathsInsideRoot &&
    binding.allAnnotationPathsInsideRoot && binding.gitTracked &&
    requiredInputExclusions.all binding.excludedInputs.contains

inductive TerminalState where
  | admitted
  | failed
  | blocked
  deriving DecidableEq, Repr

structure AdmissionTerminal where
  state : TerminalState
  reasonKind : Option Nat
  deriving DecidableEq, Repr

def terminalAdmitted (terminal : AdmissionTerminal) : Bool :=
  terminal.state == .admitted && terminal.reasonKind.isNone

structure ContractCandidate where
  schemaIdentityCurrent : Bool
  allStandardProfilesPresent : Bool
  modulesAdmitted : Bool
  binding : ProductBinding
  compilationReceipt : CompilationReceipt
  materialization : MaterializationBinding
  terminal : AdmissionTerminal
  deriving DecidableEq, Repr

def contractAdmitted
    (expected : ProductBinding)
    (independentlyAdmitted : List CompilationReceipt)
    (candidate : ContractCandidate) : Bool :=
  candidate.schemaIdentityCurrent && candidate.allStandardProfilesPresent &&
    candidate.modulesAdmitted && candidate.binding == expected &&
    projectWorkspaceIdentityAdmitted candidate.binding.projectWorkspace.identity &&
    compilationReceiptMatches candidate.binding independentlyAdmitted
      candidate.compilationReceipt &&
    materializationAdmitted candidate.materialization &&
    terminalAdmitted candidate.terminal

def manifestBoundContractAdmitted
    (expected : ProductBinding)
    (independentlyAdmitted : List CompilationReceipt)
    (manifest : ProjectWorkspaceManifestCandidate)
    (candidate : ContractCandidate) : Bool :=
  projectWorkspaceManifestAdmitted manifest &&
    manifest.binding == expected.projectWorkspace &&
    contractAdmitted expected independentlyAdmitted candidate

def bindingA : ProductBinding :=
  ⟨⟨⟨⟨1⟩, ⟨2⟩, .durableGit, .crossMachine⟩, ⟨3⟩⟩,
    ⟨11⟩, ⟨12⟩, ⟨13⟩, ⟨14⟩, ⟨15⟩,
    ⟨16⟩, ⟨17⟩, ⟨18⟩, ⟨19⟩, ⟨20⟩, ⟨21⟩⟩

def manifestA : ProjectWorkspaceManifestCandidate :=
  ⟨1, true, true, .topologyManifest, bindingA.projectWorkspace⟩

def compilationA : CompilationReceipt :=
  ⟨⟨90⟩, .admitted, bindingA.schemeProgramDigest,
    bindingA.compiledProgramAbiDigest, bindingA.mrrBundleIdentity⟩

def materializationA : MaterializationBinding :=
  ⟨true, true, true, true, true, requiredInputExclusions, true⟩

def candidateA : ContractCandidate :=
  ⟨true, true, true, bindingA, compilationA, materializationA,
    ⟨.admitted, none⟩⟩

theorem exact_contract_candidate_is_admitted :
    contractAdmitted bindingA [compilationA] candidateA = true := by
  decide

theorem exact_parser_owned_manifest_is_admitted :
    projectWorkspaceManifestAdmitted manifestA = true := by
  decide

theorem duplicate_manifest_declarations_are_rejected :
    projectWorkspaceManifestAdmitted
      { manifestA with declarationCount := 2 } = false := by
  decide

theorem host_derived_binding_cannot_replace_the_manifest :
    projectWorkspaceManifestAdmitted
      { manifestA with source := .hostDerived } = false := by
  decide

theorem runtime_generated_binding_cannot_replace_the_manifest :
    projectWorkspaceManifestAdmitted
      { manifestA with source := .runtimeGenerated } = false := by
  decide

theorem parser_ownership_is_required_for_manifest_admission :
    projectWorkspaceManifestAdmitted
      { manifestA with parserOwned := false } = false := by
  decide

theorem exact_contract_is_bound_to_the_exact_manifest :
    manifestBoundContractAdmitted bindingA [compilationA] manifestA candidateA = true := by
  decide

theorem another_valid_workspace_cannot_bypass_the_manifest :
    let alternateIdentity : ProjectWorkspaceIdentity :=
      ⟨⟨91⟩, ⟨92⟩, .durableGit, .crossMachine⟩
    let alternateManifest : ProjectWorkspaceManifestCandidate :=
      { manifestA with
          binding := { bindingA.projectWorkspace with identity := alternateIdentity } }
    projectWorkspaceIdentityAdmitted alternateIdentity = true ∧
      manifestBoundContractAdmitted bindingA [compilationA]
        alternateManifest candidateA = false := by
  decide

theorem compilation_receipt_must_be_independently_admitted :
    contractAdmitted bindingA [] candidateA = false := by
  decide

theorem admitted_terminal_cannot_carry_a_failure_reason :
    terminalAdmitted ⟨.admitted, some 7⟩ = false := by
  decide

theorem self_index_exclusion_is_required :
    materializationAdmitted
      { materializationA with
        excludedInputs := [.runtimeCache, .gitMetadata] } = false := by
  decide

theorem source_identity_drift_rejects_the_complete_contract :
    let stale : ProductBinding :=
      { bindingA with sourceSnapshotDigest := ⟨99⟩ }
    contractAdmitted stale [compilationA] candidateA = false := by
  decide

theorem workspace_root_boundary_drift_rejects_the_complete_contract :
    let movedRoot : ProductBinding :=
      { bindingA with
          projectWorkspace :=
            { bindingA.projectWorkspace with workspaceRoot := ⟨99⟩ } }
    contractAdmitted movedRoot [compilationA] candidateA = false := by
  decide

theorem topology_closure_drift_rejects_the_complete_contract :
    let changed : ProductBinding :=
      { bindingA with topologyClosureDigest := ⟨99⟩ }
    contractAdmitted changed [compilationA] candidateA = false := by
  decide

theorem compiled_abi_drift_rejects_the_compilation_receipt :
    let changed : ProductBinding :=
      { bindingA with compiledProgramAbiDigest := ⟨99⟩ }
    compilationReceiptMatches changed [compilationA] compilationA = false := by
  decide

def bindingWithReceiptCollision : ProductBinding :=
  { bindingA with compiledProgramAbiDigest := ⟨99⟩ }

def compilationWithCollidingId : CompilationReceipt :=
  { compilationA with
      compiledProgramAbiDigest := bindingWithReceiptCollision.compiledProgramAbiDigest }

def candidateWithReceiptCollision : ContractCandidate :=
  { candidateA with
      binding := bindingWithReceiptCollision
      compilationReceipt := compilationWithCollidingId }

theorem compilation_receipt_id_collision_does_not_authorize_changed_content :
    contractAdmitted bindingWithReceiptCollision [compilationA]
      candidateWithReceiptCollision = false := by
  decide

structure WorktreeObservation where
  projectWorkspaceIdentity : ProjectWorkspaceIdentity
  sourceSnapshotDigest : Identity .sourceSnapshot
  worktreeInstanceId : Identity .worktreeInstance
  deriving DecidableEq, Repr

def sameAdmittedSource
    (left right : WorktreeObservation) : Bool :=
  left.projectWorkspaceIdentity == right.projectWorkspaceIdentity &&
    left.sourceSnapshotDigest == right.sourceSnapshotDigest

def worktreeA : WorktreeObservation :=
  ⟨bindingA.projectWorkspace.identity, bindingA.sourceSnapshotDigest, ⟨301⟩⟩

def worktreeB : WorktreeObservation :=
  ⟨bindingA.projectWorkspace.identity, bindingA.sourceSnapshotDigest, ⟨302⟩⟩

theorem distinct_host_local_worktrees_can_bind_the_same_admitted_source :
    worktreeA.worktreeInstanceId ≠ worktreeB.worktreeInstanceId ∧
      sameAdmittedSource worktreeA worktreeB = true := by
  decide

theorem local_file_locator_cannot_claim_cross_machine_portability :
    projectWorkspaceIdentityAdmitted
      ⟨⟨1⟩, ⟨2⟩, .localFile, .crossMachine⟩ = false := by
  decide

theorem activation_generation_is_not_a_product_identity_field :
    exactTopologyBinding topologyBefore topologyWithStaleSource = false := by
  decide

end ASPProof.ProjectTopologyIdentityRefinement

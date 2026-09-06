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
  | workspace
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
  | compilationReceipt
  deriving DecidableEq, Repr

structure Identity (domain : IdentityDomain) where
  value : Nat
  deriving DecidableEq, Repr

structure ProductBinding where
  workspaceIdentity : Identity .workspace
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
    (independentlyAdmitted : List (Identity .compilationReceipt))
    (receipt : CompilationReceipt) : Bool :=
  receipt.state == .admitted &&
    independentlyAdmitted.contains receipt.identity &&
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
    (independentlyAdmitted : List (Identity .compilationReceipt))
    (candidate : ContractCandidate) : Bool :=
  candidate.schemaIdentityCurrent && candidate.allStandardProfilesPresent &&
    candidate.modulesAdmitted && candidate.binding == expected &&
    compilationReceiptMatches candidate.binding independentlyAdmitted
      candidate.compilationReceipt &&
    materializationAdmitted candidate.materialization &&
    terminalAdmitted candidate.terminal

def bindingA : ProductBinding :=
  ⟨⟨1⟩, ⟨11⟩, ⟨12⟩, ⟨13⟩, ⟨14⟩, ⟨15⟩,
    ⟨16⟩, ⟨17⟩, ⟨18⟩, ⟨19⟩, ⟨20⟩⟩

def compilationA : CompilationReceipt :=
  ⟨⟨90⟩, .admitted, bindingA.schemeProgramDigest,
    bindingA.compiledProgramAbiDigest, bindingA.mrrBundleIdentity⟩

def materializationA : MaterializationBinding :=
  ⟨true, true, true, true, true, requiredInputExclusions, true⟩

def candidateA : ContractCandidate :=
  ⟨true, true, true, bindingA, compilationA, materializationA,
    ⟨.admitted, none⟩⟩

theorem exact_contract_candidate_is_admitted :
    contractAdmitted bindingA [compilationA.identity] candidateA = true := by
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
    contractAdmitted stale [compilationA.identity] candidateA = false := by
  decide

theorem compiled_abi_drift_rejects_the_compilation_receipt :
    let changed : ProductBinding :=
      { bindingA with compiledProgramAbiDigest := ⟨99⟩ }
    compilationReceiptMatches changed [compilationA.identity] compilationA = false := by
  decide

theorem activation_generation_is_not_a_product_identity_field :
    exactTopologyBinding topologyBefore topologyWithStaleSource = false := by
  decide

end ASPProof.ProjectTopologyIdentityRefinement

namespace ASPProof.GraphTurboResidentIpcAuthority

structure GenerationDigest where
  value : String
  deriving DecidableEq

structure RootDigest where
  value : String
  deriving DecidableEq

structure ResidentRequest where
  requestId : Nat
  workspaceIdentity : String
  generationDigest : GenerationDigest

structure ResidentReceipt where
  requestId : Nat
  workspaceIdentity : String
  generationDigest : GenerationDigest
  processSpawns : Nat
  generationLoads : Nat

structure ManagedResidentArtifact where
  bundleRoot : String
  bundleDigest : String
  executableLocator : String
  executableDigest : String

structure ResidentArtifactConfig where
  artifactKind : String
  runtimeArtifactDigest : String
  executionArtifactLocator : String
  executionArtifactDigest : String

def configRefinesArtifact
    (artifact : ManagedResidentArtifact)
    (config : ResidentArtifactConfig) : Prop :=
  config.artifactKind = "standalone-directory" ∧
    config.runtimeArtifactDigest = artifact.bundleDigest ∧
    config.executionArtifactLocator = artifact.executableLocator ∧
    config.executionArtifactDigest = artifact.executableDigest

def refines (request : ResidentRequest) (receipt : ResidentReceipt) : Prop :=
  receipt.requestId = request.requestId ∧
    receipt.workspaceIdentity = request.workspaceIdentity ∧
    receipt.generationDigest = request.generationDigest

def warmReuse (receipt : ResidentReceipt) : Prop :=
  receipt.processSpawns = 0 ∧ receipt.generationLoads = 0

theorem root_digest_cannot_refine_generation_identity
    (request : ResidentRequest)
    (receipt : ResidentReceipt)
    (root : RootDigest)
    (different : root.value ≠ request.generationDigest.value)
    (substituted : receipt.generationDigest.value = root.value) :
    ¬ refines request receipt := by
  intro refinement
  have sameGeneration : receipt.generationDigest = request.generationDigest := refinement.2.2
  have sameValue : receipt.generationDigest.value = request.generationDigest.value :=
    congrArg GenerationDigest.value sameGeneration
  exact different (substituted ▸ sameValue)

theorem warm_reuse_has_no_process_or_generation_reload
    (receipt : ResidentReceipt)
    (warm : warmReuse receipt) :
    receipt.processSpawns + receipt.generationLoads = 0 := by
  simp [warmReuse] at warm
  omega

theorem interpreter_fallback_cannot_refine_standalone_artifact
    (artifact : ManagedResidentArtifact)
    (config : ResidentArtifactConfig)
    (fallbackLocator : String)
    (outsideBundle : fallbackLocator ≠ artifact.executableLocator)
    (fallbackSelected : config.executionArtifactLocator = fallbackLocator) :
    ¬ configRefinesArtifact artifact config := by
  intro refinement
  exact outsideBundle (fallbackSelected ▸ refinement.2.2.1)

end ASPProof.GraphTurboResidentIpcAuthority

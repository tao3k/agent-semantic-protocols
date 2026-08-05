import ASPProof.AgentSessionLifecycleProduct

namespace ASPProof.SubagentManagerProjectionDispatch

open ASPProof.AgentSessionLifecycleProduct

inductive HostPlatform where
  | codex
  | claude
  deriving Repr, DecidableEq, BEq

structure CanonicalAgentSpec where
  agentId : Nat
  specDigest : Nat
  modelPolicyDigest : Nat
  rolesDigest : Nat
  permissionPolicyDigest : Nat
  sessionLifetimeDigest : Nat
  deriving Repr, DecidableEq, BEq

structure HostProjection where
  platform : HostPlatform
  agentId : Nat
  specDigest : Nat
  modelPolicyDigest : Nat
  rolesDigest : Nat
  permissionPolicyDigest : Nat
  resolvedModel : String
  rendererDigest : Nat
  projectionDigest : Nat
  parserValid : Bool
  deriving Repr, DecidableEq, BEq

structure ProjectionPublicationReceipt where
  platform : HostPlatform
  specDigest : Nat
  projectionDigest : Nat
  previousProjectionDigest : Option Nat
  parserValidated : Bool
  atomicallyPublished : Bool
  deriving Repr, DecidableEq, BEq

structure ManagerDispatchReceipt where
  agentId : Nat
  specDigest : Nat
  projectionDigest : Nat
  physicalGeneration : Nat
  delivered : Bool
  deriving Repr, DecidableEq, BEq

def projectionRefines
    (spec : CanonicalAgentSpec)
    (projection : HostProjection) : Bool :=
  projection.agentId == spec.agentId &&
    projection.specDigest == spec.specDigest &&
    projection.modelPolicyDigest == spec.modelPolicyDigest &&
    projection.rolesDigest == spec.rolesDigest &&
    projection.permissionPolicyDigest == spec.permissionPolicyDigest &&
    !projection.resolvedModel.isEmpty &&
    projection.parserValid

def publicationAdmitted
    (spec : CanonicalAgentSpec)
    (projection : HostProjection)
    (receipt : ProjectionPublicationReceipt) : Bool :=
  projectionRefines spec projection &&
    receipt.platform == projection.platform &&
    receipt.specDigest == spec.specDigest &&
    receipt.projectionDigest == projection.projectionDigest &&
    receipt.parserValidated &&
    receipt.atomicallyPublished

def dispatchAdmitted
    (spec : CanonicalAgentSpec)
    (projection : HostProjection)
    (publication : ProjectionPublicationReceipt)
    (session : LifecycleProduct)
    (dispatch : ManagerDispatchReceipt) : Bool :=
  publicationAdmitted spec projection publication &&
    durableDispatchAuthorized session &&
    dispatch.agentId == spec.agentId &&
    dispatch.specDigest == spec.specDigest &&
    dispatch.projectionDigest == projection.projectionDigest &&
    dispatch.physicalGeneration == session.session.generation &&
    dispatch.delivered

def explorerSpec : CanonicalAgentSpec :=
  { agentId := 17
    specDigest := 7001
    modelPolicyDigest := 5600
    rolesDigest := 301
    permissionPolicyDigest := 401
    sessionLifetimeDigest := 501 }

def codexExplorerProjection : HostProjection :=
  { platform := .codex
    agentId := 17
    specDigest := 7001
    modelPolicyDigest := 5600
    rolesDigest := 301
    permissionPolicyDigest := 401
    resolvedModel := "gpt-5.6-luna"
    rendererDigest := 6101
    projectionDigest := 7101
    parserValid := true }

def claudeExplorerProjection : HostProjection :=
  { platform := .claude
    agentId := 17
    specDigest := 7001
    modelPolicyDigest := 5600
    rolesDigest := 301
    permissionPolicyDigest := 401
    resolvedModel := "platform-resolved-cheap-explorer"
    rendererDigest := 6102
    projectionDigest := 7102
    parserValid := true }

def codexPublication : ProjectionPublicationReceipt :=
  { platform := .codex
    specDigest := 7001
    projectionDigest := 7101
    previousProjectionDigest := some 7000
    parserValidated := true
    atomicallyPublished := true }

def currentExplorerSession : LifecycleProduct :=
  { server := { epoch := 1, health := .ready }
    session := { generation := 9, phase := .active }
    binding := {
      generation := 9
      childId := 17
      canonicalTarget := 101
      phase := .fresh
      terminationReceiptIndexed := false
      pathReleaseReceiptIndexed := false }
    dispatch := { generation := 9, dispatchKey := 7101, phase := .idle } }

def codexDispatch : ManagerDispatchReceipt :=
  { agentId := 17
    specDigest := 7001
    projectionDigest := 7101
    physicalGeneration := 9
    delivered := true }

theorem codex_projection_refines_canonical_agent_spec :
    projectionRefines explorerSpec codexExplorerProjection = true := by
  native_decide

theorem claude_projection_refines_the_same_canonical_agent_spec :
    projectionRefines explorerSpec claudeExplorerProjection = true := by
  native_decide

theorem current_published_projection_can_dispatch_existing_agent_session :
    dispatchAdmitted explorerSpec codexExplorerProjection codexPublication
      currentExplorerSession codexDispatch = true := by
  native_decide

def smokeModelPolicyProjection : HostProjection :=
  { codexExplorerProjection with modelPolicyDigest := 5400 }

def smokeResolvedModelProjection : HostProjection :=
  { smokeModelPolicyProjection with resolvedModel := "gpt-5.4-mini" }

def hardcodedSmokeProjection : HostProjection :=
  { smokeResolvedModelProjection with projectionDigest := 7200 }

theorem smoke_model_not_derived_from_catalog_is_rejected :
    projectionRefines explorerSpec hardcodedSmokeProjection = false := by
  native_decide

def staleSpecPublication : ProjectionPublicationReceipt :=
  { codexPublication with specDigest := 6900 }

def stalePublication : ProjectionPublicationReceipt :=
  { staleSpecPublication with projectionDigest := 7000 }

theorem stale_installed_projection_cannot_dispatch :
    dispatchAdmitted explorerSpec codexExplorerProjection stalePublication
      currentExplorerSession codexDispatch = false := by
  native_decide

def staleProfileSession : LifecycleProduct :=
  { currentExplorerSession with
    binding := { currentExplorerSession.binding with phase := .stale } }

theorem stale_resident_child_is_not_reused :
    dispatchAdmitted explorerSpec codexExplorerProjection codexPublication
      staleProfileSession codexDispatch = false := by
  native_decide

def nonAtomicPublication : ProjectionPublicationReceipt :=
  { codexPublication with atomicallyPublished := false }

theorem partial_publication_cannot_authorize_dispatch :
    dispatchAdmitted explorerSpec codexExplorerProjection nonAtomicPublication
      currentExplorerSession codexDispatch = false := by
  native_decide

def mismatchedDispatchReceipt : ManagerDispatchReceipt :=
  { codexDispatch with projectionDigest := 7000 }

theorem dispatch_receipt_must_name_live_projection :
    dispatchAdmitted explorerSpec codexExplorerProjection codexPublication
      currentExplorerSession mismatchedDispatchReceipt = false := by
  native_decide

end ASPProof.SubagentManagerProjectionDispatch

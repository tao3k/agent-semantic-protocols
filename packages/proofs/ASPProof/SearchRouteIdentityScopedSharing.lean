namespace ASPProof.SearchRouteIdentityScopedSharing

structure EvidenceIdentity where
  atomDigest : Nat
  contentDigest : Nat
  evidenceRootDigest : Nat
  providerDigest : Nat
  sourceIndexGeneration : Nat
  projectionVersion : Nat
  deriving DecidableEq, Repr

def Shareable
    (left right : EvidenceIdentity) : Prop :=
  left = right

theorem shareable_iff_complete_identity_equal
    (left right : EvidenceIdentity) :
    Shareable left right ↔ left = right :=
  Iff.rfl

def treeTokenCost
    (cost : EvidenceIdentity → Nat) :
    List EvidenceIdentity → Nat
  | [] => 0
  | identity :: rest => cost identity + treeTokenCost cost rest

def dagTokenCost
    (cost : EvidenceIdentity → Nat) :
    List EvidenceIdentity → List EvidenceIdentity → Nat
  | [], _ => 0
  | identity :: rest, seen =>
      if identity ∈ seen then
        dagTokenCost cost rest seen
      else
        cost identity + dagTokenCost cost rest (identity :: seen)

def uniqueTokenCost
    (cost : EvidenceIdentity → Nat)
    (identities : List EvidenceIdentity) : Nat :=
  dagTokenCost cost identities []

def tenTokenEvidenceCost (_identity : EvidenceIdentity) : Nat :=
  10

def sharedIdentity : EvidenceIdentity :=
  {
    atomDigest := 1
    contentDigest := 11
    evidenceRootDigest := 21
    providerDigest := 31
    sourceIndexGeneration := 7
    projectionVersion := 1
  }

def nextGenerationIdentity : EvidenceIdentity :=
  { sharedIdentity with sourceIndexGeneration := 8 }

theorem repeated_identity_tree_costs_twenty :
    treeTokenCost
      tenTokenEvidenceCost
      [sharedIdentity, sharedIdentity] = 20 := by
  decide

theorem repeated_identity_dag_costs_ten :
    uniqueTokenCost
      tenTokenEvidenceCost
      [sharedIdentity, sharedIdentity] = 10 := by
  decide

theorem equal_atom_label_does_not_imply_shareable :
    sharedIdentity.atomDigest = nextGenerationIdentity.atomDigest ∧
      ¬ Shareable sharedIdentity nextGenerationIdentity := by
  unfold Shareable sharedIdentity nextGenerationIdentity
  decide

theorem generation_drift_prevents_deduplication :
    uniqueTokenCost
      tenTokenEvidenceCost
      [sharedIdentity, nextGenerationIdentity] = 20 := by
  decide

structure RequestUse where
  requestDigest : Nat
  evidenceIdentity : EvidenceIdentity
  promptTokens : Nat
  deriving DecidableEq, Repr

def requestPromptTokenCost : List RequestUse → Nat
  | [] => 0
  | request :: rest =>
      request.promptTokens + requestPromptTokenCost rest

def firstRequest : RequestUse :=
  {
    requestDigest := 1001
    evidenceIdentity := sharedIdentity
    promptTokens := 5
  }

def secondRequest : RequestUse :=
  {
    requestDigest := 1002
    evidenceIdentity := sharedIdentity
    promptTokens := 5
  }

theorem distinct_requests_can_share_semantic_identity :
    firstRequest.requestDigest ≠ secondRequest.requestDigest ∧
      firstRequest.evidenceIdentity =
        secondRequest.evidenceIdentity := by
  decide

theorem shared_evidence_does_not_dedup_request_prompt_cost :
    requestPromptTokenCost [firstRequest, secondRequest] = 10 := by
  decide

theorem semantic_and_prompt_charges_remain_separate :
    uniqueTokenCost
        tenTokenEvidenceCost
        [firstRequest.evidenceIdentity, secondRequest.evidenceIdentity] = 10 ∧
      requestPromptTokenCost [firstRequest, secondRequest] = 10 := by
  exact
    ⟨
      repeated_identity_dag_costs_ten,
      shared_evidence_does_not_dedup_request_prompt_cost
    ⟩

end ASPProof.SearchRouteIdentityScopedSharing

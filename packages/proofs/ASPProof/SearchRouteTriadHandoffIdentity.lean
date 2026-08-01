import Std

namespace ASPProof.SearchRouteTriadHandoffIdentity

structure HandoffEvidence where
  selectorChain : Nat
  pythonChain : Nat
  leanChain : Nat
  goldenChain : Nat
  auditChain : Nat
  configurationReady : Bool
  identityAuthenticated : Bool
  selectorReady : Bool
  pythonReady : Bool
  leanReady : Bool
  goldenReady : Bool
  auditReady : Bool
  normalizedAuditReplayReady : Bool
  deriving DecidableEq, Repr

def SameReceiptChain (evidence : HandoffEvidence) : Prop :=
  evidence.selectorChain = evidence.pythonChain ∧
    evidence.selectorChain = evidence.leanChain ∧
    evidence.selectorChain = evidence.goldenChain ∧
    evidence.selectorChain = evidence.auditChain

def AllGatesReady (evidence : HandoffEvidence) : Prop :=
  evidence.configurationReady = true ∧
    evidence.identityAuthenticated = true ∧
    evidence.selectorReady = true ∧
    evidence.pythonReady = true ∧
    evidence.leanReady = true ∧
    evidence.goldenReady = true ∧
    evidence.auditReady = true ∧
    evidence.normalizedAuditReplayReady = true

def HandoffAdmitted (evidence : HandoffEvidence) : Prop :=
  SameReceiptChain evidence ∧ AllGatesReady evidence

theorem admitted_handoff_has_one_receipt_chain
    {evidence : HandoffEvidence}
    (admitted : HandoffAdmitted evidence) :
    SameReceiptChain evidence :=
  admitted.1

theorem admitted_handoff_has_all_gates
    {evidence : HandoffEvidence}
    (admitted : HandoffAdmitted evidence) :
    AllGatesReady evidence :=
  admitted.2

theorem same_chain_and_ready_gates_admit
    {evidence : HandoffEvidence}
    (sameChain : SameReceiptChain evidence)
    (allReady : AllGatesReady evidence) :
    HandoffAdmitted evidence :=
  ⟨sameChain, allReady⟩

theorem invalid_execution_configuration_blocks_admission
    {evidence : HandoffEvidence}
    (invalidConfiguration : evidence.configurationReady = false) :
    ¬ HandoffAdmitted evidence := by
  intro admitted
  have contradiction : false = true :=
    invalidConfiguration.symm.trans admitted.2.1
  exact Bool.noConfusion contradiction

theorem unauthenticated_chain_identity_blocks_admission
    {evidence : HandoffEvidence}
    (unauthenticated : evidence.identityAuthenticated = false) :
    ¬ HandoffAdmitted evidence := by
  intro admitted
  have contradiction : false = true :=
    unauthenticated.symm.trans admitted.2.2.1
  exact Bool.noConfusion contradiction

theorem unreplayed_normalized_audit_blocks_admission
    {evidence : HandoffEvidence}
    (unreplayed : evidence.normalizedAuditReplayReady = false) :
    ¬ HandoffAdmitted evidence := by
  intro admitted
  have contradiction : false = true :=
    unreplayed.symm.trans admitted.2.2.2.2.2.2.2.2
  exact Bool.noConfusion contradiction

def splicedAllGreenEvidence : HandoffEvidence where
  selectorChain := 1
  pythonChain := 1
  leanChain := 2
  goldenChain := 1
  auditChain := 1
  configurationReady := true
  identityAuthenticated := true
  selectorReady := true
  pythonReady := true
  leanReady := true
  goldenReady := true
  auditReady := true
  normalizedAuditReplayReady := true

def fullyReadyEvidence : HandoffEvidence where
  selectorChain := 3
  pythonChain := 3
  leanChain := 3
  goldenChain := 3
  auditChain := 3
  configurationReady := true
  identityAuthenticated := true
  selectorReady := true
  pythonReady := true
  leanReady := true
  goldenReady := true
  auditReady := true
  normalizedAuditReplayReady := true

def observedV1BlockedEvidence : HandoffEvidence where
  selectorChain := 3
  pythonChain := 3
  leanChain := 3
  goldenChain := 3
  auditChain := 3
  configurationReady := false
  identityAuthenticated := true
  selectorReady := true
  pythonReady := true
  leanReady := true
  goldenReady := true
  auditReady := true
  normalizedAuditReplayReady := true

def generatorOnlyEvidence : HandoffEvidence where
  selectorChain := 3
  pythonChain := 3
  leanChain := 3
  goldenChain := 3
  auditChain := 3
  configurationReady := true
  identityAuthenticated := true
  selectorReady := true
  pythonReady := true
  leanReady := true
  goldenReady := true
  auditReady := true
  normalizedAuditReplayReady := false

theorem all_green_but_spliced_receipts_are_not_admitted :
    ¬ HandoffAdmitted splicedAllGreenEvidence := by
  unfold HandoffAdmitted SameReceiptChain AllGatesReady
  unfold splicedAllGreenEvidence
  decide

theorem fully_ready_handoff_is_admitted :
    HandoffAdmitted fullyReadyEvidence :=
  ⟨⟨rfl, rfl, rfl, rfl⟩, ⟨rfl, rfl, rfl, rfl, rfl, rfl, rfl, rfl⟩⟩

theorem generator_only_audit_is_not_admitted :
    ¬ HandoffAdmitted generatorOnlyEvidence :=
  unreplayed_normalized_audit_blocks_admission rfl

theorem observed_v1_preflight_failure_is_not_admitted :
    ¬ HandoffAdmitted observedV1BlockedEvidence :=
  invalid_execution_configuration_blocks_admission rfl

end ASPProof.SearchRouteTriadHandoffIdentity

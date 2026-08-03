namespace ASPProof.PolyglotSearchEvidenceSufficiency

structure Evidence where
  candidateExecutable : Bool
  candidateArtifactBound : Bool
  pairedTaskIdentity : Bool
  pairedSourceSnapshot : Bool
  pairedEnvironment : Bool
  rawMetricsComplete : Bool
  coldCacheCovered : Bool
  warmCacheCovered : Bool
  repeatedSamples : Bool
  qualificationPassed : Bool
  deriving DecidableEq

def pairedEvidenceComplete (evidence : Evidence) : Prop :=
  evidence.pairedTaskIdentity = true ∧
  evidence.pairedSourceSnapshot = true ∧
  evidence.pairedEnvironment = true ∧
  evidence.rawMetricsComplete = true ∧
  evidence.coldCacheCovered = true ∧
  evidence.warmCacheCovered = true ∧
  evidence.repeatedSamples = true

instance (evidence : Evidence) : Decidable (pairedEvidenceComplete evidence) := by
  unfold pairedEvidenceComplete
  infer_instance

def evidenceConsistent (evidence : Evidence) : Prop :=
  (evidence.candidateArtifactBound = true → evidence.candidateExecutable = true) ∧
  (pairedEvidenceComplete evidence →
    evidence.candidateExecutable = true ∧ evidence.candidateArtifactBound = true) ∧
  (evidence.qualificationPassed = true →
    evidence.candidateExecutable = true ∧
    evidence.candidateArtifactBound = true ∧
    pairedEvidenceComplete evidence)

instance (evidence : Evidence) : Decidable (evidenceConsistent evidence) := by
  unfold evidenceConsistent
  infer_instance

def readyForPairedRuns (evidence : Evidence) : Prop :=
  evidenceConsistent evidence ∧
  evidence.candidateExecutable = true ∧
  evidence.candidateArtifactBound = true

instance (evidence : Evidence) : Decidable (readyForPairedRuns evidence) := by
  unfold readyForPairedRuns
  infer_instance

def qualificationAuthorized (evidence : Evidence) : Prop :=
  evidenceConsistent evidence ∧
  evidence.candidateExecutable = true ∧
  evidence.candidateArtifactBound = true ∧
  pairedEvidenceComplete evidence ∧
  evidence.qualificationPassed = true

instance (evidence : Evidence) : Decidable (qualificationAuthorized evidence) := by
  unfold qualificationAuthorized
  infer_instance

def referenceOnlyEvidence : Evidence where
  candidateExecutable := false
  candidateArtifactBound := false
  pairedTaskIdentity := false
  pairedSourceSnapshot := false
  pairedEnvironment := false
  rawMetricsComplete := false
  coldCacheCovered := false
  warmCacheCovered := false
  repeatedSamples := false
  qualificationPassed := false

def executableOnlyEvidence : Evidence :=
  { referenceOnlyEvidence with candidateExecutable := true }

def pairedButUnqualifiedEvidence : Evidence where
  candidateExecutable := true
  candidateArtifactBound := true
  pairedTaskIdentity := true
  pairedSourceSnapshot := true
  pairedEnvironment := true
  rawMetricsComplete := true
  coldCacheCovered := true
  warmCacheCovered := true
  repeatedSamples := true
  qualificationPassed := false

def completeEvidence : Evidence :=
  { pairedButUnqualifiedEvidence with qualificationPassed := true }

def inconsistentDownstreamClaim : Evidence :=
  { referenceOnlyEvidence with qualificationPassed := true }

theorem reference_only_evidence_cannot_authorize_replacement :
    ¬qualificationAuthorized referenceOnlyEvidence := by
  decide

theorem executable_candidate_alone_is_insufficient :
    ¬readyForPairedRuns executableOnlyEvidence ∧
      ¬qualificationAuthorized executableOnlyEvidence := by
  decide

theorem paired_measurements_without_qualification_remain_blocked :
    readyForPairedRuns pairedButUnqualifiedEvidence ∧
      ¬qualificationAuthorized pairedButUnqualifiedEvidence := by
  decide

theorem complete_evidence_authorizes_qualification :
    qualificationAuthorized completeEvidence := by
  decide

theorem authorization_exposes_every_prerequisite
    {evidence : Evidence} (h : qualificationAuthorized evidence) :
    evidence.candidateExecutable = true ∧
    evidence.candidateArtifactBound = true ∧
    pairedEvidenceComplete evidence ∧
    evidence.qualificationPassed = true := by
  exact ⟨h.2.1, h.2.2.1, h.2.2.2.1, h.2.2.2.2⟩

theorem downstream_claim_without_prerequisites_is_inconsistent :
    ¬evidenceConsistent inconsistentDownstreamClaim := by
  decide

end ASPProof.PolyglotSearchEvidenceSufficiency

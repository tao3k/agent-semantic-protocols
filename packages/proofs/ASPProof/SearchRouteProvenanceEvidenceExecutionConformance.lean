-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteVerifierArtifactProvenance

namespace ASPProof.SearchRouteProvenanceEvidenceExecutionConformance

open ASPProof.SearchRouteExecutableVerifierConformance
open ASPProof.SearchRouteVerifierArtifactProvenance

/-!
RFC 01.18 closes the structural artifact-provenance conjunction.  Its Boolean
decision fields remain claims until a claim-kind-aware verifier is resolved,
invoked, decoded, and independently replayed.  This module supplies that
execution closure and a temporal transparency-checkpoint transition.
-/

inductive ProvenanceClaimKind
  | sourceImmutable
  | recipeHermetic
  | buildAttestation
  | transparencyInclusion
  | checkpointConsistency
  | checkpointFreshness
  | independentRebuild
  | outputComparison
  deriving DecidableEq, Repr

structure ExecutableEvidenceClaimContext where
  claimKind : ProvenanceClaimKind
  subjectDigest : Nat
  policyDigest : Nat
  serializerDigest : Nat
  statementSchemaDigest : Nat
  verifierIdDigest : Nat
  verifierBinaryDigest : Nat
  decoderSchemaDigest : Nat
  deriving DecidableEq, Repr

structure CanonicalEvidenceClaimReceipt where
  claimKind : ProvenanceClaimKind
  subjectDigest : Nat
  policyDigest : Nat
  statementBytesDigest : Nat
  serializerDigest : Nat
  statementSchemaDigest : Nat
  canonical : Bool
  deriving DecidableEq, Repr

structure EvidenceClaimVerifierResolutionReceipt where
  claimKind : ProvenanceClaimKind
  policyDigest : Nat
  verifierIdDigest : Nat
  verifierBinaryDigest : Nat
  resolved : Bool
  deriving DecidableEq, Repr

structure EvidenceClaimInvocationReceipt where
  claimKind : ProvenanceClaimKind
  subjectDigest : Nat
  policyDigest : Nat
  statementBytesDigest : Nat
  verifierBinaryDigest : Nat
  executorDigest : Nat
  responseBytesDigest : Nat
  exitCode : Nat
  deriving DecidableEq, Repr

structure DecodedEvidenceClaimReceipt where
  claimKind : ProvenanceClaimKind
  subjectDigest : Nat
  policyDigest : Nat
  responseBytesDigest : Nat
  decoderSchemaDigest : Nat
  decoded : Bool
  accepted : Bool
  deriving DecidableEq, Repr

structure IndependentEvidenceClaimReplayReceipt where
  claimKind : ProvenanceClaimKind
  subjectDigest : Nat
  policyDigest : Nat
  statementBytesDigest : Nat
  verifierBinaryDigest : Nat
  executorDigest : Nat
  replayed : Bool
  accepted : Bool
  deriving DecidableEq, Repr

structure ExecutableEvidenceClaimReceipt where
  statement : CanonicalEvidenceClaimReceipt
  resolution : EvidenceClaimVerifierResolutionReceipt
  invocation : EvidenceClaimInvocationReceipt
  decoded : DecodedEvidenceClaimReceipt
  replay : IndependentEvidenceClaimReplayReceipt
  deriving DecidableEq, Repr

def EvidenceClaimStatementConformant
    (context : ExecutableEvidenceClaimContext)
    (receipt : CanonicalEvidenceClaimReceipt) : Prop :=
  receipt.claimKind = context.claimKind ∧
  receipt.subjectDigest = context.subjectDigest ∧
  receipt.policyDigest = context.policyDigest ∧
  receipt.serializerDigest = context.serializerDigest ∧
  receipt.statementSchemaDigest = context.statementSchemaDigest ∧
  receipt.canonical = true

def EvidenceClaimResolutionConformant
    (context : ExecutableEvidenceClaimContext)
    (receipt : EvidenceClaimVerifierResolutionReceipt) : Prop :=
  receipt.claimKind = context.claimKind ∧
  receipt.policyDigest = context.policyDigest ∧
  receipt.verifierIdDigest = context.verifierIdDigest ∧
  receipt.verifierBinaryDigest = context.verifierBinaryDigest ∧
  receipt.resolved = true

def EvidenceClaimInvocationConformant
    (statement : CanonicalEvidenceClaimReceipt)
    (resolution : EvidenceClaimVerifierResolutionReceipt)
    (receipt : EvidenceClaimInvocationReceipt) : Prop :=
  receipt.claimKind = statement.claimKind ∧
  receipt.subjectDigest = statement.subjectDigest ∧
  receipt.policyDigest = statement.policyDigest ∧
  receipt.statementBytesDigest = statement.statementBytesDigest ∧
  receipt.verifierBinaryDigest = resolution.verifierBinaryDigest ∧
  receipt.exitCode = 0

def EvidenceClaimDecodeConformant
    (context : ExecutableEvidenceClaimContext)
    (invocation : EvidenceClaimInvocationReceipt)
    (receipt : DecodedEvidenceClaimReceipt) : Prop :=
  receipt.claimKind = invocation.claimKind ∧
  receipt.subjectDigest = invocation.subjectDigest ∧
  receipt.policyDigest = invocation.policyDigest ∧
  receipt.responseBytesDigest = invocation.responseBytesDigest ∧
  receipt.decoderSchemaDigest = context.decoderSchemaDigest ∧
  receipt.decoded = true ∧
  receipt.accepted = true

def EvidenceClaimReplayConformant
    (statement : CanonicalEvidenceClaimReceipt)
    (invocation : EvidenceClaimInvocationReceipt)
    (decoded : DecodedEvidenceClaimReceipt)
    (receipt : IndependentEvidenceClaimReplayReceipt) : Prop :=
  receipt.claimKind = statement.claimKind ∧
  receipt.subjectDigest = statement.subjectDigest ∧
  receipt.policyDigest = statement.policyDigest ∧
  receipt.statementBytesDigest = statement.statementBytesDigest ∧
  receipt.verifierBinaryDigest = invocation.verifierBinaryDigest ∧
  receipt.executorDigest ≠ invocation.executorDigest ∧
  receipt.replayed = true ∧
  receipt.accepted = decoded.accepted

def ValidExecutableEvidenceClaim
    (context : ExecutableEvidenceClaimContext)
    (receipt : ExecutableEvidenceClaimReceipt) : Prop :=
  EvidenceClaimStatementConformant context receipt.statement ∧
  EvidenceClaimResolutionConformant context receipt.resolution ∧
  EvidenceClaimInvocationConformant
    receipt.statement receipt.resolution receipt.invocation ∧
  EvidenceClaimDecodeConformant context receipt.invocation receipt.decoded ∧
  EvidenceClaimReplayConformant
    receipt.statement receipt.invocation receipt.decoded receipt.replay

def SubjectOnlyEvidence
    (subjectDigest : Nat)
    (receipt : ExecutableEvidenceClaimReceipt) : Prop :=
  receipt.statement.subjectDigest = subjectDigest

def BooleanDecisionPrefix
    (receipt : ExecutableEvidenceClaimReceipt) : Prop :=
  receipt.decoded.accepted = true

structure TransparencyTemporalContext where
  logDigest : Nat
  now : Nat
  maximumCheckpointAge : Nat
  minimumTreeSize : Nat
  deriving DecidableEq, Repr

structure TransparencyCheckpoint where
  logDigest : Nat
  checkpointDigest : Nat
  rootDigest : Nat
  treeSize : Nat
  issuedAt : Nat
  deriving DecidableEq, Repr

structure TransparencyCheckpointTransition where
  prior : TransparencyCheckpoint
  current : TransparencyCheckpoint
  transitionStatementDigest : Nat
  deriving DecidableEq, Repr

def TransparencyCheckpointTransitionConformant
    (context : TransparencyTemporalContext)
    (transition : TransparencyCheckpointTransition) : Prop :=
  transition.prior.logDigest = context.logDigest ∧
  transition.current.logDigest = context.logDigest ∧
  transition.prior.treeSize ≤ transition.current.treeSize ∧
  transition.prior.issuedAt < transition.current.issuedAt ∧
  transition.current.issuedAt ≤ context.now ∧
  context.now ≤ transition.current.issuedAt + context.maximumCheckpointAge ∧
  context.minimumTreeSize ≤ transition.current.treeSize

structure ExecutableProvenanceEvidenceContext where
  policyDigest : Nat
  temporal : TransparencyTemporalContext
  sourceImmutable : ExecutableEvidenceClaimContext
  recipeHermetic : ExecutableEvidenceClaimContext
  buildAttestation : ExecutableEvidenceClaimContext
  transparencyInclusion : ExecutableEvidenceClaimContext
  checkpointConsistency : ExecutableEvidenceClaimContext
  checkpointFreshness : ExecutableEvidenceClaimContext
  independentRebuild : ExecutableEvidenceClaimContext
  outputComparison : ExecutableEvidenceClaimContext
  deriving DecidableEq, Repr

structure ExecutableProvenanceEvidenceReceipt where
  checkpointTransition : TransparencyCheckpointTransition
  sourceImmutable : ExecutableEvidenceClaimReceipt
  recipeHermetic : ExecutableEvidenceClaimReceipt
  buildAttestation : ExecutableEvidenceClaimReceipt
  transparencyInclusion : ExecutableEvidenceClaimReceipt
  checkpointConsistency : ExecutableEvidenceClaimReceipt
  checkpointFreshness : ExecutableEvidenceClaimReceipt
  independentRebuild : ExecutableEvidenceClaimReceipt
  outputComparison : ExecutableEvidenceClaimReceipt
  deriving DecidableEq, Repr

def ClaimTargetConformant
    (policyDigest : Nat)
    (expectedKind : ProvenanceClaimKind)
    (expectedSubjectDigest : Nat)
    (context : ExecutableEvidenceClaimContext) : Prop :=
  context.claimKind = expectedKind ∧
  context.subjectDigest = expectedSubjectDigest ∧
  context.policyDigest = policyDigest

def CheckpointBindsProvenance
    (provenance : VerifierArtifactProvenanceReceipt)
    (transition : TransparencyCheckpointTransition) : Prop :=
  transition.current.logDigest = provenance.transparency.logDigest ∧
  transition.current.checkpointDigest = provenance.transparency.checkpointDigest ∧
  transition.current.rootDigest = provenance.transparency.rootDigest ∧
  transition.current.treeSize = provenance.transparency.treeSize

def ProvenanceClaimTargetsConformant
    (context : ExecutableProvenanceEvidenceContext)
    (provenance : VerifierArtifactProvenanceReceipt)
    (transition : TransparencyCheckpointTransition) : Prop :=
  ClaimTargetConformant context.policyDigest .sourceImmutable
    provenance.source.snapshotDigest context.sourceImmutable ∧
  ClaimTargetConformant context.policyDigest .recipeHermetic
    provenance.recipe.recipeDigest context.recipeHermetic ∧
  ClaimTargetConformant context.policyDigest .buildAttestation
    provenance.build.provenanceStatementDigest context.buildAttestation ∧
  ClaimTargetConformant context.policyDigest .transparencyInclusion
    provenance.transparency.rootDigest context.transparencyInclusion ∧
  ClaimTargetConformant context.policyDigest .checkpointConsistency
    transition.transitionStatementDigest context.checkpointConsistency ∧
  ClaimTargetConformant context.policyDigest .checkpointFreshness
    transition.current.checkpointDigest context.checkpointFreshness ∧
  ClaimTargetConformant context.policyDigest .independentRebuild
    provenance.rebuild.verifierBinaryDigest context.independentRebuild ∧
  ClaimTargetConformant context.policyDigest .outputComparison
    provenance.build.verifierBinaryDigest context.outputComparison

def AllProvenanceClaimsExecutable
    (context : ExecutableProvenanceEvidenceContext)
    (receipt : ExecutableProvenanceEvidenceReceipt) : Prop :=
  ValidExecutableEvidenceClaim context.sourceImmutable receipt.sourceImmutable ∧
  ValidExecutableEvidenceClaim context.recipeHermetic receipt.recipeHermetic ∧
  ValidExecutableEvidenceClaim context.buildAttestation receipt.buildAttestation ∧
  ValidExecutableEvidenceClaim
    context.transparencyInclusion receipt.transparencyInclusion ∧
  ValidExecutableEvidenceClaim
    context.checkpointConsistency receipt.checkpointConsistency ∧
  ValidExecutableEvidenceClaim
    context.checkpointFreshness receipt.checkpointFreshness ∧
  ValidExecutableEvidenceClaim context.independentRebuild receipt.independentRebuild ∧
  ValidExecutableEvidenceClaim context.outputComparison receipt.outputComparison

def ValidExecutableProvenanceEvidence
    (provenanceContext : VerifierArtifactProvenanceContext)
    (resolution : VerifierResolutionReceipt)
    (executionContext : ExecutableProvenanceEvidenceContext)
    (provenance : VerifierArtifactProvenanceReceipt)
    (execution : ExecutableProvenanceEvidenceReceipt) : Prop :=
  ValidVerifierArtifactProvenance provenanceContext resolution provenance ∧
  CheckpointBindsProvenance provenance execution.checkpointTransition ∧
  TransparencyCheckpointTransitionConformant
    executionContext.temporal execution.checkpointTransition ∧
  ProvenanceClaimTargetsConformant
    executionContext provenance execution.checkpointTransition ∧
  AllProvenanceClaimsExecutable executionContext execution

def ExecutionBackedRouteNoWorse
    (evidenceValid : Prop)
    (left right : ProvenanceGuardedRouteCandidate) : Prop :=
  evidenceValid ∧ RouteCostNoWorse left right

theorem valid_executable_evidence_claim_binds_all_stages
    {context : ExecutableEvidenceClaimContext}
    {receipt : ExecutableEvidenceClaimReceipt}
    (valid : ValidExecutableEvidenceClaim context receipt) :
    EvidenceClaimStatementConformant context receipt.statement ∧
    EvidenceClaimResolutionConformant context receipt.resolution ∧
    EvidenceClaimInvocationConformant
      receipt.statement receipt.resolution receipt.invocation ∧
    EvidenceClaimDecodeConformant context receipt.invocation receipt.decoded ∧
    EvidenceClaimReplayConformant
      receipt.statement receipt.invocation receipt.decoded receipt.replay :=
  valid

theorem valid_executable_evidence_claim_binds_kind_policy_subject
    {context : ExecutableEvidenceClaimContext}
    {receipt : ExecutableEvidenceClaimReceipt}
    (valid : ValidExecutableEvidenceClaim context receipt) :
    receipt.statement.claimKind = context.claimKind ∧
    receipt.statement.policyDigest = context.policyDigest ∧
    receipt.statement.subjectDigest = context.subjectDigest :=
  ⟨valid.1.1, valid.1.2.2.1, valid.1.2.1⟩

theorem valid_executable_evidence_claim_binds_resolution
    {context : ExecutableEvidenceClaimContext}
    {receipt : ExecutableEvidenceClaimReceipt}
    (valid : ValidExecutableEvidenceClaim context receipt) :
    EvidenceClaimResolutionConformant context receipt.resolution :=
  valid.2.1

theorem valid_executable_evidence_claim_binds_invocation_and_decode
    {context : ExecutableEvidenceClaimContext}
    {receipt : ExecutableEvidenceClaimReceipt}
    (valid : ValidExecutableEvidenceClaim context receipt) :
    EvidenceClaimInvocationConformant
        receipt.statement receipt.resolution receipt.invocation ∧
    EvidenceClaimDecodeConformant context receipt.invocation receipt.decoded :=
  ⟨valid.2.2.1, valid.2.2.2.1⟩

theorem valid_executable_evidence_claim_has_distinct_replay_executor
    {context : ExecutableEvidenceClaimContext}
    {receipt : ExecutableEvidenceClaimReceipt}
    (valid : ValidExecutableEvidenceClaim context receipt) :
    receipt.replay.executorDigest ≠ receipt.invocation.executorDigest :=
  by
    rcases valid with ⟨_, _, _, _, replay⟩
    exact replay.2.2.2.2.2.1

theorem valid_executable_provenance_evidence_projects_structural_provenance
    {provenanceContext : VerifierArtifactProvenanceContext}
    {resolution : VerifierResolutionReceipt}
    {executionContext : ExecutableProvenanceEvidenceContext}
    {provenance : VerifierArtifactProvenanceReceipt}
    {execution : ExecutableProvenanceEvidenceReceipt}
    (valid : ValidExecutableProvenanceEvidence
      provenanceContext resolution executionContext provenance execution) :
    ValidVerifierArtifactProvenance provenanceContext resolution provenance :=
  valid.1

theorem valid_executable_provenance_evidence_binds_all_claim_targets
    {provenanceContext : VerifierArtifactProvenanceContext}
    {resolution : VerifierResolutionReceipt}
    {executionContext : ExecutableProvenanceEvidenceContext}
    {provenance : VerifierArtifactProvenanceReceipt}
    {execution : ExecutableProvenanceEvidenceReceipt}
    (valid : ValidExecutableProvenanceEvidence
      provenanceContext resolution executionContext provenance execution) :
    ProvenanceClaimTargetsConformant
      executionContext provenance execution.checkpointTransition :=
  valid.2.2.2.1

theorem valid_executable_provenance_evidence_binds_current_checkpoint
    {provenanceContext : VerifierArtifactProvenanceContext}
    {resolution : VerifierResolutionReceipt}
    {executionContext : ExecutableProvenanceEvidenceContext}
    {provenance : VerifierArtifactProvenanceReceipt}
    {execution : ExecutableProvenanceEvidenceReceipt}
    (valid : ValidExecutableProvenanceEvidence
      provenanceContext resolution executionContext provenance execution) :
    CheckpointBindsProvenance provenance execution.checkpointTransition :=
  valid.2.1

theorem valid_executable_provenance_evidence_has_monotone_fresh_checkpoint
    {provenanceContext : VerifierArtifactProvenanceContext}
    {resolution : VerifierResolutionReceipt}
    {executionContext : ExecutableProvenanceEvidenceContext}
    {provenance : VerifierArtifactProvenanceReceipt}
    {execution : ExecutableProvenanceEvidenceReceipt}
    (valid : ValidExecutableProvenanceEvidence
      provenanceContext resolution executionContext provenance execution) :
    execution.checkpointTransition.prior.treeSize ≤
        execution.checkpointTransition.current.treeSize ∧
    execution.checkpointTransition.prior.issuedAt <
        execution.checkpointTransition.current.issuedAt ∧
    executionContext.temporal.now ≤
        execution.checkpointTransition.current.issuedAt +
          executionContext.temporal.maximumCheckpointAge := by
  rcases valid.2.2.1 with ⟨_, _, monotone, timeMonotone, _, fresh, _⟩
  exact ⟨monotone, timeMonotone, fresh⟩

theorem invalid_execution_backing_cannot_win_by_lower_cost
    {evidenceValid : Prop}
    {left right : ProvenanceGuardedRouteCandidate}
    (invalid : ¬ evidenceValid) :
    ¬ ExecutionBackedRouteNoWorse evidenceValid left right := by
  intro admitted
  exact invalid admitted.1

def exampleClaimContext
    (kind : ProvenanceClaimKind)
    (subjectDigest : Nat) : ExecutableEvidenceClaimContext where
  claimKind := kind
  subjectDigest := subjectDigest
  policyDigest := 700
  serializerDigest := 701
  statementSchemaDigest := 702
  verifierIdDigest := 703
  verifierBinaryDigest := 704
  decoderSchemaDigest := 705

def exampleClaimReceipt
    (kind : ProvenanceClaimKind)
    (subjectDigest : Nat) : ExecutableEvidenceClaimReceipt where
  statement := {
    claimKind := kind
    subjectDigest := subjectDigest
    policyDigest := 700
    statementBytesDigest := 801
    serializerDigest := 701
    statementSchemaDigest := 702
    canonical := true
  }
  resolution := {
    claimKind := kind
    policyDigest := 700
    verifierIdDigest := 703
    verifierBinaryDigest := 704
    resolved := true
  }
  invocation := {
    claimKind := kind
    subjectDigest := subjectDigest
    policyDigest := 700
    statementBytesDigest := 801
    verifierBinaryDigest := 704
    executorDigest := 901
    responseBytesDigest := 802
    exitCode := 0
  }
  decoded := {
    claimKind := kind
    subjectDigest := subjectDigest
    policyDigest := 700
    responseBytesDigest := 802
    decoderSchemaDigest := 705
    decoded := true
    accepted := true
  }
  replay := {
    claimKind := kind
    subjectDigest := subjectDigest
    policyDigest := 700
    statementBytesDigest := 801
    verifierBinaryDigest := 704
    executorDigest := 902
    replayed := true
    accepted := true
  }

def exampleTemporalContext : TransparencyTemporalContext where
  logDigest := 501
  now := 1015
  maximumCheckpointAge := 10
  minimumTreeSize := 10

def exampleCheckpointTransition : TransparencyCheckpointTransition where
  prior := {
    logDigest := 501
    checkpointDigest := 499
    rootDigest := 500
    treeSize := 8
    issuedAt := 1000
  }
  current := {
    logDigest := 501
    checkpointDigest := 502
    rootDigest := 503
    treeSize := 12
    issuedAt := 1010
  }
  transitionStatementDigest := 504

def exampleExecutionContext : ExecutableProvenanceEvidenceContext where
  policyDigest := 700
  temporal := exampleTemporalContext
  sourceImmutable := exampleClaimContext .sourceImmutable 103
  recipeHermetic := exampleClaimContext .recipeHermetic 201
  buildAttestation := exampleClaimContext .buildAttestation 401
  transparencyInclusion := exampleClaimContext .transparencyInclusion 503
  checkpointConsistency := exampleClaimContext .checkpointConsistency 504
  checkpointFreshness := exampleClaimContext .checkpointFreshness 502
  independentRebuild := exampleClaimContext .independentRebuild 602
  outputComparison := exampleClaimContext .outputComparison 602

def exampleExecutionReceipt : ExecutableProvenanceEvidenceReceipt where
  checkpointTransition := exampleCheckpointTransition
  sourceImmutable := exampleClaimReceipt .sourceImmutable 103
  recipeHermetic := exampleClaimReceipt .recipeHermetic 201
  buildAttestation := exampleClaimReceipt .buildAttestation 401
  transparencyInclusion := exampleClaimReceipt .transparencyInclusion 503
  checkpointConsistency := exampleClaimReceipt .checkpointConsistency 504
  checkpointFreshness := exampleClaimReceipt .checkpointFreshness 502
  independentRebuild := exampleClaimReceipt .independentRebuild 602
  outputComparison := exampleClaimReceipt .outputComparison 602

theorem example_executable_provenance_evidence_is_valid :
    ValidExecutableProvenanceEvidence
      exampleProvenanceContext
      exampleResolution
      exampleExecutionContext
      exampleProvenanceReceipt
      exampleExecutionReceipt := by
  simp [
    ValidExecutableProvenanceEvidence,
    CheckpointBindsProvenance,
    TransparencyCheckpointTransitionConformant,
    ProvenanceClaimTargetsConformant,
    ClaimTargetConformant,
    AllProvenanceClaimsExecutable,
    ValidExecutableEvidenceClaim,
    EvidenceClaimStatementConformant,
    EvidenceClaimResolutionConformant,
    EvidenceClaimInvocationConformant,
    EvidenceClaimDecodeConformant,
    EvidenceClaimReplayConformant,
    exampleExecutionContext,
    exampleExecutionReceipt,
    exampleCheckpointTransition,
    exampleTemporalContext,
    exampleClaimContext,
    exampleClaimReceipt,
    exampleProvenanceReceipt,
    exampleProvenanceContext,
    exampleResolution,
    exampleSource,
    exampleRecipe,
    exampleBuild,
    exampleTransparency,
    exampleRebuild,
    ValidVerifierArtifactProvenance,
    SourceSnapshotConformant,
    BuildRecipeConformant,
    BuildAttestationConformant,
    TransparencyInclusionConformant,
    IndependentRebuildConformant
  ]

def unresolvedSourceClaimExecution : ExecutableProvenanceEvidenceReceipt :=
  { exampleExecutionReceipt with
    sourceImmutable := {
      exampleExecutionReceipt.sourceImmutable with
      resolution := {
        exampleExecutionReceipt.sourceImmutable.resolution with resolved := false
      }
    }
  }

theorem boolean_provenance_only_does_not_prove_executable_evidence :
    ValidVerifierArtifactProvenance
      exampleProvenanceContext exampleResolution exampleProvenanceReceipt ∧
    ¬ ValidExecutableProvenanceEvidence
      exampleProvenanceContext
      exampleResolution
      exampleExecutionContext
      exampleProvenanceReceipt
      unresolvedSourceClaimExecution := by
  constructor
  · exact example_provenance_is_valid
  · simp [
      ValidExecutableProvenanceEvidence,
      AllProvenanceClaimsExecutable,
      ValidExecutableEvidenceClaim,
      EvidenceClaimResolutionConformant,
      unresolvedSourceClaimExecution,
      exampleExecutionReceipt,
      exampleClaimReceipt
    ]

def wrongKindSourceClaim : ExecutableEvidenceClaimReceipt :=
  exampleClaimReceipt .recipeHermetic 103

theorem subject_only_binding_allows_cross_kind_replay :
    SubjectOnlyEvidence 103 wrongKindSourceClaim ∧
    ¬ ValidExecutableEvidenceClaim
      (exampleClaimContext .sourceImmutable 103) wrongKindSourceClaim := by
  simp [
    SubjectOnlyEvidence,
    ValidExecutableEvidenceClaim,
    EvidenceClaimStatementConformant,
    wrongKindSourceClaim,
    exampleClaimContext,
    exampleClaimReceipt
  ]

def crossPolicySourceClaim : ExecutableEvidenceClaimReceipt :=
  let base := exampleClaimReceipt .sourceImmutable 103
  {
    statement := { base.statement with policyDigest := 999 }
    resolution := { base.resolution with policyDigest := 999 }
    invocation := { base.invocation with policyDigest := 999 }
    decoded := { base.decoded with policyDigest := 999 }
    replay := { base.replay with policyDigest := 999 }
  }

theorem cross_policy_claim_replay_is_rejected :
    ¬ ValidExecutableEvidenceClaim
      (exampleClaimContext .sourceImmutable 103) crossPolicySourceClaim := by
  simp [
    ValidExecutableEvidenceClaim,
    EvidenceClaimStatementConformant,
    crossPolicySourceClaim,
    exampleClaimContext,
    exampleClaimReceipt
  ]

def sameExecutorSourceClaim : ExecutableEvidenceClaimReceipt :=
  let base := exampleClaimReceipt .sourceImmutable 103
  { base with replay := { base.replay with executorDigest := 901 } }

theorem same_executor_claim_replay_is_rejected :
    ¬ ValidExecutableEvidenceClaim
      (exampleClaimContext .sourceImmutable 103) sameExecutorSourceClaim := by
  simp [
    ValidExecutableEvidenceClaim,
    EvidenceClaimReplayConformant,
    sameExecutorSourceClaim,
    exampleClaimContext,
    exampleClaimReceipt
  ]

def staleExecutionContext : ExecutableProvenanceEvidenceContext :=
  { exampleExecutionContext with
    temporal := { exampleTemporalContext with now := 2000 }
  }

theorem stale_checkpoint_execution_is_rejected :
    ¬ ValidExecutableProvenanceEvidence
      exampleProvenanceContext
      exampleResolution
      staleExecutionContext
      exampleProvenanceReceipt
      exampleExecutionReceipt := by
  simp [
    ValidExecutableProvenanceEvidence,
    TransparencyCheckpointTransitionConformant,
    staleExecutionContext,
    exampleExecutionContext,
    exampleTemporalContext,
    exampleExecutionReceipt,
    exampleCheckpointTransition
  ]

def rollbackExecutionReceipt : ExecutableProvenanceEvidenceReceipt :=
  { exampleExecutionReceipt with
    checkpointTransition := {
      exampleCheckpointTransition with
      current := { exampleCheckpointTransition.current with treeSize := 7 }
    }
  }

theorem checkpoint_tree_rollback_is_rejected :
    ¬ ValidExecutableProvenanceEvidence
      exampleProvenanceContext
      exampleResolution
      exampleExecutionContext
      exampleProvenanceReceipt
      rollbackExecutionReceipt := by
  simp [
    ValidExecutableProvenanceEvidence,
    CheckpointBindsProvenance,
    rollbackExecutionReceipt,
    exampleExecutionReceipt,
    exampleCheckpointTransition,
    exampleProvenanceReceipt,
    exampleTransparency
  ]

theorem accepted_boolean_does_not_prove_executable_claim :
    BooleanDecisionPrefix unresolvedSourceClaimExecution.sourceImmutable ∧
    ¬ ValidExecutableEvidenceClaim
      exampleExecutionContext.sourceImmutable
      unresolvedSourceClaimExecution.sourceImmutable := by
  simp [
    BooleanDecisionPrefix,
    ValidExecutableEvidenceClaim,
    EvidenceClaimResolutionConformant,
    unresolvedSourceClaimExecution,
    exampleExecutionReceipt,
    exampleExecutionContext,
    exampleClaimContext,
    exampleClaimReceipt
  ]

end ASPProof.SearchRouteProvenanceEvidenceExecutionConformance

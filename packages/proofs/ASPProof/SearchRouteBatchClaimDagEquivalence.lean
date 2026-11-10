-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteProvenanceEvidenceExecutionConformance

namespace ASPProof.SearchRouteBatchClaimDagEquivalence

open ASPProof.SearchRouteProvenanceEvidenceExecutionConformance

/-!
Batching must not pretend that a manifest invocation executed each claim as an
individual process.  This module gives individual and batch transports a shared
typed outcome semantics, then proves that both realizations agree on that
semantics while retaining distinct execution receipts.
-/

structure BatchExecutionKey where
  policyDigest : Nat
  serializerDigest : Nat
  statementSchemaDigest : Nat
  verifierIdDigest : Nat
  verifierBinaryDigest : Nat
  decoderSchemaDigest : Nat
  deriving DecidableEq, Repr

structure BatchClaimTarget where
  claimKind : ProvenanceClaimKind
  subjectDigest : Nat
  policyDigest : Nat
  deriving DecidableEq, Repr

structure ClaimOutcome where
  claimKind : ProvenanceClaimKind
  subjectDigest : Nat
  policyDigest : Nat
  verifierBinaryDigest : Nat
  accepted : Bool
  deriving DecidableEq, Repr

structure BatchClaimEntry where
  context : ExecutableEvidenceClaimContext
  statement : CanonicalEvidenceClaimReceipt
  deriving DecidableEq, Repr

structure BatchManifest where
  manifestDigest : Nat
  targets : List BatchClaimTarget
  canonical : Bool
  deriving DecidableEq, Repr

structure BatchVerifierResolutionReceipt where
  key : BatchExecutionKey
  resolved : Bool
  deriving DecidableEq, Repr

structure BatchInvocationReceipt where
  manifestDigest : Nat
  key : BatchExecutionKey
  executorDigest : Nat
  responseBytesDigest : Nat
  exitCode : Nat
  deriving DecidableEq, Repr

structure BatchDecodedReceipt where
  manifestDigest : Nat
  key : BatchExecutionKey
  responseBytesDigest : Nat
  outcomes : List ClaimOutcome
  decoded : Bool
  allAccepted : Bool
  deriving DecidableEq, Repr

structure BatchReplayReceipt where
  manifestDigest : Nat
  key : BatchExecutionKey
  executorDigest : Nat
  outcomes : List ClaimOutcome
  replayed : Bool
  allAccepted : Bool
  deriving DecidableEq, Repr

structure BatchClaimExecutionReceipt where
  manifest : BatchManifest
  resolution : BatchVerifierResolutionReceipt
  invocation : BatchInvocationReceipt
  decoded : BatchDecodedReceipt
  replay : BatchReplayReceipt
  deriving DecidableEq, Repr

def contextExecutionKey
    (context : ExecutableEvidenceClaimContext) : BatchExecutionKey where
  policyDigest := context.policyDigest
  serializerDigest := context.serializerDigest
  statementSchemaDigest := context.statementSchemaDigest
  verifierIdDigest := context.verifierIdDigest
  verifierBinaryDigest := context.verifierBinaryDigest
  decoderSchemaDigest := context.decoderSchemaDigest

def entryTarget (entry : BatchClaimEntry) : BatchClaimTarget where
  claimKind := entry.context.claimKind
  subjectDigest := entry.context.subjectDigest
  policyDigest := entry.context.policyDigest

def expectedClaimOutcome
    (context : ExecutableEvidenceClaimContext) : ClaimOutcome where
  claimKind := context.claimKind
  subjectDigest := context.subjectDigest
  policyDigest := context.policyDigest
  verifierBinaryDigest := context.verifierBinaryDigest
  accepted := true

def individualClaimOutcome
    (receipt : ExecutableEvidenceClaimReceipt) : ClaimOutcome where
  claimKind := receipt.statement.claimKind
  subjectDigest := receipt.statement.subjectDigest
  policyDigest := receipt.statement.policyDigest
  verifierBinaryDigest := receipt.resolution.verifierBinaryDigest
  accepted := receipt.decoded.accepted

def EntryConformant
    (key : BatchExecutionKey)
    (entry : BatchClaimEntry) : Prop :=
  contextExecutionKey entry.context = key ∧
  EvidenceClaimStatementConformant entry.context entry.statement

def BatchManifestConformant
    (entries : List BatchClaimEntry)
    (manifest : BatchManifest) : Prop :=
  entries ≠ [] ∧
  manifest.targets = entries.map entryTarget ∧
  manifest.targets.Nodup ∧
  manifest.canonical = true

def BatchResolutionConformant
    (key : BatchExecutionKey)
    (receipt : BatchVerifierResolutionReceipt) : Prop :=
  receipt.key = key ∧ receipt.resolved = true

def BatchInvocationConformant
    (manifest : BatchManifest)
    (resolution : BatchVerifierResolutionReceipt)
    (receipt : BatchInvocationReceipt) : Prop :=
  receipt.manifestDigest = manifest.manifestDigest ∧
  receipt.key = resolution.key ∧
  receipt.exitCode = 0

def BatchDecodeConformant
    (entries : List BatchClaimEntry)
    (manifest : BatchManifest)
    (invocation : BatchInvocationReceipt)
    (receipt : BatchDecodedReceipt) : Prop :=
  receipt.manifestDigest = manifest.manifestDigest ∧
  receipt.key = invocation.key ∧
  receipt.responseBytesDigest = invocation.responseBytesDigest ∧
  receipt.outcomes = entries.map (expectedClaimOutcome ∘ BatchClaimEntry.context) ∧
  receipt.decoded = true ∧
  receipt.allAccepted = true

def BatchReplayConformant
    (manifest : BatchManifest)
    (invocation : BatchInvocationReceipt)
    (decoded : BatchDecodedReceipt)
    (receipt : BatchReplayReceipt) : Prop :=
  receipt.manifestDigest = manifest.manifestDigest ∧
  receipt.key = invocation.key ∧
  receipt.executorDigest ≠ invocation.executorDigest ∧
  receipt.outcomes = decoded.outcomes ∧
  receipt.replayed = true ∧
  receipt.allAccepted = decoded.allAccepted

def ValidBatchClaimExecution
    (key : BatchExecutionKey)
    (entries : List BatchClaimEntry)
    (receipt : BatchClaimExecutionReceipt) : Prop :=
  BatchManifestConformant entries receipt.manifest ∧
  (∀ entry ∈ entries, EntryConformant key entry) ∧
  BatchResolutionConformant key receipt.resolution ∧
  BatchInvocationConformant receipt.manifest receipt.resolution receipt.invocation ∧
  BatchDecodeConformant entries receipt.manifest receipt.invocation receipt.decoded ∧
  BatchReplayConformant receipt.manifest receipt.invocation receipt.decoded receipt.replay

def AllAcceptedPrefix (receipt : BatchClaimExecutionReceipt) : Prop :=
  receipt.decoded.allAccepted = true

def SameInvocationHistory
    (individual : ExecutableEvidenceClaimReceipt)
    (batch : BatchClaimExecutionReceipt) : Prop :=
  individual.invocation.statementBytesDigest = batch.manifest.manifestDigest

structure SearchBatchCacheIdentity where
  manifestDigest : Nat
  policyDigest : Nat
  verifierBinaryDigest : Nat
  decoderSchemaDigest : Nat
  checkpointDigest : Nat
  deriving DecidableEq, Repr

structure ModelPrefixCacheIdentity where
  promptPrefixDigest : Nat
  modelDigest : Nat
  deriving DecidableEq, Repr

def SearchBatchCacheReusable
    (expected observed : SearchBatchCacheIdentity) : Prop :=
  expected = observed

def BinaryOnlySearchCacheHit
    (expected observed : SearchBatchCacheIdentity) : Prop :=
  expected.verifierBinaryDigest = observed.verifierBinaryDigest

def ModelPrefixCacheHit
    (expected observed : ModelPrefixCacheIdentity) : Prop :=
  expected = observed

def individualToolRounds (claimCount : Nat) : Nat :=
  2 * claimCount

def batchToolRounds (claimCount : Nat) : Nat :=
  if claimCount = 0 then 0 else 2

def individualGraphNodes (claimCount : Nat) : Nat :=
  claimCount + 4 * claimCount

def batchGraphNodes (claimCount : Nat) : Nat :=
  if claimCount = 0 then 0 else claimCount + 4

def individualInputTokens
    (sharedTokens perClaimTokens claimCount : Nat) : Nat :=
  claimCount * (sharedTokens + perClaimTokens)

def batchInputTokens
    (sharedTokens perClaimTokens claimCount : Nat) : Nat :=
  if claimCount = 0 then 0 else sharedTokens + claimCount * perClaimTokens

theorem valid_individual_claim_projects_expected_outcome
    {context : ExecutableEvidenceClaimContext}
    {receipt : ExecutableEvidenceClaimReceipt}
    (valid : ValidExecutableEvidenceClaim context receipt) :
    individualClaimOutcome receipt = expectedClaimOutcome context := by
  cases context
  cases receipt
  simp [
    ValidExecutableEvidenceClaim,
    EvidenceClaimStatementConformant,
    EvidenceClaimResolutionConformant,
    EvidenceClaimInvocationConformant,
    EvidenceClaimDecodeConformant,
    EvidenceClaimReplayConformant,
    individualClaimOutcome,
    expectedClaimOutcome
  ] at valid ⊢
  rcases valid with ⟨statement, resolution, _, decoded, _⟩
  rcases statement with ⟨kind, subject, policy, _, _, _⟩
  rcases resolution with ⟨_, _, _, binary, _⟩
  rcases decoded with ⟨_, _, _, _, _, _, accepted⟩
  exact ⟨kind, subject, policy, binary, accepted⟩

theorem valid_batch_projects_each_expected_outcome
    {key : BatchExecutionKey}
    {entries : List BatchClaimEntry}
    {receipt : BatchClaimExecutionReceipt}
    (valid : ValidBatchClaimExecution key entries receipt)
    {entry : BatchClaimEntry}
    (member : entry ∈ entries) :
    expectedClaimOutcome entry.context ∈ receipt.decoded.outcomes := by
  rcases valid with ⟨_, _, _, _, decoded, _⟩
  rcases decoded with ⟨_, _, _, outcomes, _, _⟩
  rw [outcomes]
  exact List.mem_map.mpr ⟨entry, member, rfl⟩

theorem valid_batch_and_individual_claims_have_equivalent_outcomes
    {key : BatchExecutionKey}
    {entries : List BatchClaimEntry}
    {batch : BatchClaimExecutionReceipt}
    {entry : BatchClaimEntry}
    {individual : ExecutableEvidenceClaimReceipt}
    (batchValid : ValidBatchClaimExecution key entries batch)
    (member : entry ∈ entries)
    (individualValid : ValidExecutableEvidenceClaim entry.context individual) :
    individualClaimOutcome individual ∈ batch.decoded.outcomes := by
  rw [valid_individual_claim_projects_expected_outcome individualValid]
  exact valid_batch_projects_each_expected_outcome batchValid member

theorem valid_batch_has_distinct_replay_executor
    {key : BatchExecutionKey}
    {entries : List BatchClaimEntry}
    {receipt : BatchClaimExecutionReceipt}
    (valid : ValidBatchClaimExecution key entries receipt) :
    receipt.replay.executorDigest ≠ receipt.invocation.executorDigest := by
  rcases valid with ⟨_, _, _, _, _, replay⟩
  exact replay.2.2.1

theorem valid_batch_has_nonempty_unique_manifest
    {key : BatchExecutionKey}
    {entries : List BatchClaimEntry}
    {receipt : BatchClaimExecutionReceipt}
    (valid : ValidBatchClaimExecution key entries receipt) :
    entries ≠ [] ∧ receipt.manifest.targets.Nodup :=
  ⟨valid.1.1, valid.1.2.2.1⟩

theorem valid_batch_binds_canonical_manifest
    {key : BatchExecutionKey}
    {entries : List BatchClaimEntry}
    {receipt : BatchClaimExecutionReceipt}
    (valid : ValidBatchClaimExecution key entries receipt) :
    BatchManifestConformant entries receipt.manifest :=
  valid.1

theorem valid_batch_entries_share_execution_key
    {key : BatchExecutionKey}
    {entries : List BatchClaimEntry}
    {receipt : BatchClaimExecutionReceipt}
    (valid : ValidBatchClaimExecution key entries receipt) :
    ∀ entry ∈ entries, contextExecutionKey entry.context = key := by
  intro entry member
  exact (valid.2.1 entry member).1

theorem batch_tool_rounds_no_more_than_individual
    {claimCount : Nat}
    (nonempty : 0 < claimCount) :
    batchToolRounds claimCount ≤ individualToolRounds claimCount := by
  have scaled := Nat.mul_le_mul_left 2 nonempty
  simpa [batchToolRounds, individualToolRounds, Nat.ne_of_gt nonempty] using scaled

theorem batch_tool_rounds_strictly_less_for_multiple_claims
    {claimCount : Nat}
    (multiple : 2 ≤ claimCount) :
    batchToolRounds claimCount < individualToolRounds claimCount := by
  cases claimCount with
  | zero => exact (Nat.not_succ_le_zero 1 multiple).elim
  | succ claimCount =>
      cases claimCount with
      | zero => exact (Nat.not_succ_le_self 1 multiple).elim
      | succ claimCount =>
          simp [batchToolRounds, individualToolRounds, Nat.mul_succ]

theorem batch_graph_nodes_no_more_than_individual
    {claimCount : Nat}
    (nonempty : 0 < claimCount) :
    batchGraphNodes claimCount ≤ individualGraphNodes claimCount := by
  have fourScaled := Nat.mul_le_mul_left 4 nonempty
  have withClaimCount := Nat.add_le_add_left fourScaled claimCount
  calc
    batchGraphNodes claimCount = claimCount + 4 := by
      simp [batchGraphNodes, Nat.ne_of_gt nonempty]
    _ ≤ claimCount + 4 * claimCount := by simpa using withClaimCount
    _ = individualGraphNodes claimCount := by rfl

theorem batch_graph_nodes_strictly_less_for_multiple_claims
    {claimCount : Nat}
    (multiple : 2 ≤ claimCount) :
    batchGraphNodes claimCount < individualGraphNodes claimCount := by
  cases claimCount with
  | zero => exact (Nat.not_succ_le_zero 1 multiple).elim
  | succ claimCount =>
      cases claimCount with
      | zero => exact (Nat.not_succ_le_self 1 multiple).elim
      | succ claimCount =>
          simp [batchGraphNodes, individualGraphNodes, Nat.mul_succ]

theorem batch_input_tokens_no_more_than_individual
    {sharedTokens perClaimTokens claimCount : Nat}
    (nonempty : 0 < claimCount) :
    batchInputTokens sharedTokens perClaimTokens claimCount ≤
      individualInputTokens sharedTokens perClaimTokens claimCount := by
  have sharedBound : sharedTokens ≤ claimCount * sharedTokens := by
    calc
      sharedTokens = 1 * sharedTokens := by simp
      _ ≤ claimCount * sharedTokens := Nat.mul_le_mul_right sharedTokens nonempty
  simp [
    batchInputTokens,
    individualInputTokens,
    Nat.ne_of_gt nonempty,
    Nat.mul_add
  ]
  exact sharedBound

def exampleBatchKey : BatchExecutionKey :=
  contextExecutionKey (exampleClaimContext .sourceImmutable 103)

def exampleSourceEntry : BatchClaimEntry where
  context := exampleClaimContext .sourceImmutable 103
  statement := (exampleClaimReceipt .sourceImmutable 103).statement

def exampleRecipeEntry : BatchClaimEntry where
  context := exampleClaimContext .recipeHermetic 201
  statement := (exampleClaimReceipt .recipeHermetic 201).statement

def exampleBatchEntries : List BatchClaimEntry :=
  [exampleSourceEntry, exampleRecipeEntry]

def exampleBatchReceipt : BatchClaimExecutionReceipt where
  manifest := {
    manifestDigest := 1001
    targets := exampleBatchEntries.map entryTarget
    canonical := true
  }
  resolution := {
    key := exampleBatchKey
    resolved := true
  }
  invocation := {
    manifestDigest := 1001
    key := exampleBatchKey
    executorDigest := 1101
    responseBytesDigest := 1002
    exitCode := 0
  }
  decoded := {
    manifestDigest := 1001
    key := exampleBatchKey
    responseBytesDigest := 1002
    outcomes := exampleBatchEntries.map (expectedClaimOutcome ∘ BatchClaimEntry.context)
    decoded := true
    allAccepted := true
  }
  replay := {
    manifestDigest := 1001
    key := exampleBatchKey
    executorDigest := 1102
    outcomes := exampleBatchEntries.map (expectedClaimOutcome ∘ BatchClaimEntry.context)
    replayed := true
    allAccepted := true
  }

theorem example_batch_is_valid :
    ValidBatchClaimExecution exampleBatchKey exampleBatchEntries exampleBatchReceipt := by
  simp [
    ValidBatchClaimExecution,
    BatchManifestConformant,
    EntryConformant,
    BatchResolutionConformant,
    BatchInvocationConformant,
    BatchDecodeConformant,
    BatchReplayConformant,
    exampleBatchKey,
    exampleBatchEntries,
    exampleSourceEntry,
    exampleRecipeEntry,
    exampleBatchReceipt,
    contextExecutionKey,
    entryTarget,
    expectedClaimOutcome,
    exampleClaimContext,
    exampleClaimReceipt,
    EvidenceClaimStatementConformant
  ]

def duplicateTargetEntries : List BatchClaimEntry :=
  [exampleSourceEntry, exampleSourceEntry]

def duplicateTargetBatch : BatchClaimExecutionReceipt :=
  { exampleBatchReceipt with
    manifest := {
      exampleBatchReceipt.manifest with
      targets := duplicateTargetEntries.map entryTarget
    }
    decoded := {
      exampleBatchReceipt.decoded with
      outcomes := duplicateTargetEntries.map
        (expectedClaimOutcome ∘ BatchClaimEntry.context)
    }
    replay := {
      exampleBatchReceipt.replay with
      outcomes := duplicateTargetEntries.map
        (expectedClaimOutcome ∘ BatchClaimEntry.context)
    }
  }

theorem duplicate_manifest_targets_are_rejected :
    ¬ ValidBatchClaimExecution exampleBatchKey duplicateTargetEntries duplicateTargetBatch := by
  simp [
    ValidBatchClaimExecution,
    BatchManifestConformant,
    duplicateTargetEntries,
    duplicateTargetBatch,
    exampleBatchReceipt,
    exampleSourceEntry,
    entryTarget,
    exampleClaimContext
  ]

def wrongPolicyRecipeEntry : BatchClaimEntry :=
  { exampleRecipeEntry with
    context := { exampleRecipeEntry.context with policyDigest := 999 }
  }

def crossPolicyEntries : List BatchClaimEntry :=
  [exampleSourceEntry, wrongPolicyRecipeEntry]

theorem cross_policy_batch_entry_is_rejected :
    ¬ (∀ entry ∈ crossPolicyEntries, EntryConformant exampleBatchKey entry) := by
  simp [
    crossPolicyEntries,
    wrongPolicyRecipeEntry,
    exampleRecipeEntry,
    exampleSourceEntry,
    EntryConformant,
    exampleBatchKey,
    contextExecutionKey,
    exampleClaimContext,
    exampleClaimReceipt,
    EvidenceClaimStatementConformant
  ]

def permutedDecisionBatch : BatchClaimExecutionReceipt :=
  { exampleBatchReceipt with
    decoded := {
      exampleBatchReceipt.decoded with
      outcomes := [
        expectedClaimOutcome exampleRecipeEntry.context,
        expectedClaimOutcome exampleSourceEntry.context
      ]
    }
    replay := {
      exampleBatchReceipt.replay with
      outcomes := [
        expectedClaimOutcome exampleRecipeEntry.context,
        expectedClaimOutcome exampleSourceEntry.context
      ]
    }
  }

theorem permuted_decision_vector_is_rejected :
    AllAcceptedPrefix permutedDecisionBatch ∧
    ¬ ValidBatchClaimExecution exampleBatchKey exampleBatchEntries permutedDecisionBatch := by
  simp [
    AllAcceptedPrefix,
    ValidBatchClaimExecution,
    BatchDecodeConformant,
    permutedDecisionBatch,
    exampleBatchReceipt,
    exampleBatchEntries,
    exampleSourceEntry,
    exampleRecipeEntry,
    expectedClaimOutcome,
    exampleClaimContext
  ]

def wrongManifestInvocationBatch : BatchClaimExecutionReceipt :=
  { exampleBatchReceipt with
    invocation := {
      exampleBatchReceipt.invocation with manifestDigest := 9999
    }
  }

theorem wrong_manifest_invocation_is_rejected :
    ¬ ValidBatchClaimExecution
      exampleBatchKey exampleBatchEntries wrongManifestInvocationBatch := by
  simp [
    ValidBatchClaimExecution,
    BatchInvocationConformant,
    wrongManifestInvocationBatch,
    exampleBatchReceipt
  ]

def sameExecutorBatchReplay : BatchClaimExecutionReceipt :=
  { exampleBatchReceipt with
    replay := { exampleBatchReceipt.replay with executorDigest := 1101 }
  }

theorem same_executor_batch_replay_is_rejected :
    ¬ ValidBatchClaimExecution
      exampleBatchKey exampleBatchEntries sameExecutorBatchReplay := by
  simp [
    ValidBatchClaimExecution,
    BatchReplayConformant,
    sameExecutorBatchReplay,
    exampleBatchReceipt
  ]

def expectedSearchCache : SearchBatchCacheIdentity where
  manifestDigest := 1001
  policyDigest := 700
  verifierBinaryDigest := 704
  decoderSchemaDigest := 705
  checkpointDigest := 502

def wrongManifestSearchCache : SearchBatchCacheIdentity :=
  { expectedSearchCache with manifestDigest := 9999 }

theorem binary_only_cache_hit_does_not_prove_search_batch_reuse :
    BinaryOnlySearchCacheHit expectedSearchCache wrongManifestSearchCache ∧
    ¬ SearchBatchCacheReusable expectedSearchCache wrongManifestSearchCache := by
  simp [
    BinaryOnlySearchCacheHit,
    SearchBatchCacheReusable,
    expectedSearchCache,
    wrongManifestSearchCache
  ]

def exampleModelPrefixCache : ModelPrefixCacheIdentity where
  promptPrefixDigest := 1201
  modelDigest := 1202

theorem model_prefix_cache_hit_does_not_prove_search_evidence_reuse :
    ModelPrefixCacheHit exampleModelPrefixCache exampleModelPrefixCache ∧
    ¬ SearchBatchCacheReusable expectedSearchCache wrongManifestSearchCache := by
  simp [
    ModelPrefixCacheHit,
    SearchBatchCacheReusable,
    exampleModelPrefixCache,
    expectedSearchCache,
    wrongManifestSearchCache
  ]

def exampleIndividualSourceReceipt : ExecutableEvidenceClaimReceipt :=
  exampleClaimReceipt .sourceImmutable 103

theorem batch_semantic_equivalence_does_not_fabricate_individual_invocation :
    ValidExecutableEvidenceClaim
        exampleSourceEntry.context exampleIndividualSourceReceipt ∧
    ValidBatchClaimExecution exampleBatchKey exampleBatchEntries exampleBatchReceipt ∧
    ¬ SameInvocationHistory exampleIndividualSourceReceipt exampleBatchReceipt := by
  constructor
  · simp [
      ValidExecutableEvidenceClaim,
      EvidenceClaimStatementConformant,
      EvidenceClaimResolutionConformant,
      EvidenceClaimInvocationConformant,
      EvidenceClaimDecodeConformant,
      EvidenceClaimReplayConformant,
      exampleSourceEntry,
      exampleIndividualSourceReceipt,
      exampleClaimContext,
      exampleClaimReceipt
    ]
  · exact ⟨example_batch_is_valid, by
      simp [
        SameInvocationHistory,
        exampleIndividualSourceReceipt,
        exampleClaimReceipt,
        exampleBatchReceipt
      ]⟩

end ASPProof.SearchRouteBatchClaimDagEquivalence

-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.MutationOwnerDeltaAtomicity

def candidateDomain : String := "workspace-mutation-candidate-v1\u0000"

def candidatePreimage (publishedSourceRootDigest : String) : String :=
  candidateDomain ++ publishedSourceRootDigest

structure PublicationTrace where
  baseEpoch : Nat
  targetEpoch : Nat
  publishedEpochs : List Nat
  publishedSourceRootDigest : String
  candidateDigestPreimage : String
  memoryBackendEpoch : Nat
  durableGenerationEpoch : Nat
  exactProjectionEpoch : Nat
  searchAuthorityEpoch : Nat
  activePointerEpoch : Nat
  terminalReady : Bool

def RefinesV1 (trace : PublicationTrace) : Prop :=
  trace.targetEpoch = trace.baseEpoch + 1 ∧
  trace.publishedEpochs = [trace.targetEpoch] ∧
  trace.candidateDigestPreimage = candidatePreimage trace.publishedSourceRootDigest ∧
  trace.memoryBackendEpoch = trace.targetEpoch ∧
  trace.durableGenerationEpoch = trace.targetEpoch ∧
  trace.exactProjectionEpoch = trace.targetEpoch ∧
  trace.searchAuthorityEpoch = trace.targetEpoch ∧
  trace.activePointerEpoch = trace.targetEpoch ∧
  trace.terminalReady = true

theorem publishes_exactly_one_epoch
    (trace : PublicationTrace)
    (h : RefinesV1 trace) :
    trace.publishedEpochs = [trace.targetEpoch] :=
  by
    rcases h with ⟨_, hEpochs, _, _, _, _, _, _, _⟩
    exact hEpochs

theorem candidate_identity_is_source_root_bound
    (trace : PublicationTrace)
    (h : RefinesV1 trace) :
    trace.candidateDigestPreimage = candidatePreimage trace.publishedSourceRootDigest :=
  by
    rcases h with ⟨_, _, hCandidate, _, _, _, _, _, _⟩
    exact hCandidate

theorem ready_implies_search_authority_is_current
    (trace : PublicationTrace)
    (h : RefinesV1 trace) :
    trace.terminalReady = true ∧ trace.searchAuthorityEpoch = trace.targetEpoch :=
  by
    rcases h with ⟨_, _, _, _, _, _, hSearch, _, hReady⟩
    exact ⟨hReady, hSearch⟩

theorem ready_linearizes_all_generation_authorities
    (trace : PublicationTrace)
    (h : RefinesV1 trace) :
    trace.memoryBackendEpoch = trace.targetEpoch ∧
    trace.durableGenerationEpoch = trace.targetEpoch ∧
    trace.exactProjectionEpoch = trace.targetEpoch ∧
    trace.searchAuthorityEpoch = trace.targetEpoch ∧
    trace.activePointerEpoch = trace.targetEpoch :=
  by
    rcases h with ⟨_, _, _, hMemory, hDurable, hExact, hSearch, hPointer, _⟩
    exact ⟨hMemory, hDurable, hExact, hSearch, hPointer⟩

end ASPProof.MutationOwnerDeltaAtomicity

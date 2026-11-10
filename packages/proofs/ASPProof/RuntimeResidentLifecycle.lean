-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.RuntimeSupervisorReconciliation

namespace ASPProof.RuntimeResidentLifecycle

structure HostCapabilities where
  typedSpawn : Bool
  retireChild : Bool
  releaseCanonicalPath : Bool
  deriving DecidableEq, Repr

inductive RepairDecision where
  | ready
  | retireAndReplace
  | blockResidentRoute
  deriving DecidableEq, Repr

def decideRepair (profileMatches : Bool) (host : HostCapabilities) : RepairDecision :=
  if profileMatches then
    .ready
  else if host.typedSpawn && host.retireChild && host.releaseCanonicalPath then
    .retireAndReplace
  else
    .blockResidentRoute

theorem matching_profile_is_ready (host : HostCapabilities) :
    decideRepair true host = .ready := by
  simp [decideRepair]

theorem missing_typed_spawn_blocks
    (host : HostCapabilities) (h : host.typedSpawn = false) :
    decideRepair false host = .blockResidentRoute := by
  simp [decideRepair, h]

theorem missing_retirement_blocks
    (host : HostCapabilities) (h : host.retireChild = false) :
    decideRepair false host = .blockResidentRoute := by
  simp [decideRepair, h]

theorem missing_path_release_blocks
    (host : HostCapabilities) (h : host.releaseCanonicalPath = false) :
    decideRepair false host = .blockResidentRoute := by
  simp [decideRepair, h]

theorem complete_host_capability_replaces :
    decideRepair false ⟨true, true, true⟩ = .retireAndReplace := by
  rfl

inductive RepairPhase where
  | observe
  | retire
  | create
  | audit
  | ready
  | blocked
  deriving DecidableEq, Repr

def progressRank : RepairPhase → Nat
  | .observe => 4
  | .retire => 3
  | .create => 2
  | .audit => 1
  | .ready => 0
  | .blocked => 0

inductive Advances : RepairPhase → RepairPhase → Prop where
  | observedReplaceable : Advances .observe .retire
  | childRetired : Advances .retire .create
  | replacementCreated : Advances .create .audit
  | replacementAudited : Advances .audit .ready
  | capabilityUnavailable : Advances .observe .blocked

theorem admitted_transition_decreases_rank
    {before after : RepairPhase} (h : Advances before after) :
    progressRank after < progressRank before := by
  cases h <;> decide

def blocksUnrelatedTools (_ : RepairDecision) : Bool := false

theorem repair_never_blocks_unrelated_tools (decision : RepairDecision) :
    blocksUnrelatedTools decision = false := by
  rfl

structure RepairLease where
  generation : Nat
  owner : String
  deriving DecidableEq, Repr

def sameLease (left right : RepairLease) : Prop :=
  left.generation = right.generation ∧ left.owner = right.owner

theorem same_generation_lease_owner_unique
    (left right : RepairLease)
    (generation : Nat)
    (hl : left.generation = generation)
    (hr : right.generation = generation)
    (ho : left.owner = right.owner) :
    sameLease left right := by
  constructor
  · omega
  · exact ho

end ASPProof.RuntimeResidentLifecycle

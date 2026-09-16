-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import Std.Tactic

namespace ASPProof.RuntimeSchedulerOwnership

inductive Component where
  | workspaceScheduler
  | clientDb
  | runtimeServer
deriving DecidableEq, Repr

inductive Authority where
  | runtimeProfile
  | resourceAdmission
  | taskLifecycle
  | databaseGeneration
deriving DecidableEq, Repr

def Owns : Component → Authority → Prop
  | .workspaceScheduler, .runtimeProfile => True
  | .workspaceScheduler, .resourceAdmission => True
  | .workspaceScheduler, .taskLifecycle => True
  | .clientDb, .databaseGeneration => True
  | _, _ => False

structure CompileCost where
  scheduler : Nat
  clientDb : Nat
  runtimeServer : Nat
deriving DecidableEq, Repr

def oldQueryGenerationClosure (cost : CompileCost) : Nat :=
  cost.runtimeServer + cost.clientDb + cost.scheduler

def directSchedulerClosure (cost : CompileCost) : Nat :=
  cost.runtimeServer + cost.scheduler

theorem client_db_does_not_own_runtime_profile :
    ¬ Owns .clientDb .runtimeProfile := by
  simp [Owns]

theorem client_db_does_not_own_resource_admission :
    ¬ Owns .clientDb .resourceAdmission := by
  simp [Owns]

theorem client_db_does_not_own_task_lifecycle :
    ¬ Owns .clientDb .taskLifecycle := by
  simp [Owns]

theorem scheduler_extraction_never_increases_query_generation_closure
    (cost : CompileCost) :
    directSchedulerClosure cost ≤ oldQueryGenerationClosure cost := by
  simp [directSchedulerClosure, oldQueryGenerationClosure]

theorem scheduler_extraction_strictly_reduces_positive_client_db_closure
    (cost : CompileCost) (positiveClientDb : 0 < cost.clientDb) :
    directSchedulerClosure cost < oldQueryGenerationClosure cost := by
  simp [directSchedulerClosure, oldQueryGenerationClosure]
  omega

structure StageResources where
  cpuHeld : Nat
  memoryHeld : Nat
deriving DecidableEq, Repr

def releaseCpu (resources : StageResources) : StageResources :=
  { resources with cpuHeld := 0 }

def CanAdmitNext
    (cpuCapacity memoryCapacity : Nat)
    (retained next : StageResources) : Prop :=
  retained.cpuHeld + next.cpuHeld ≤ cpuCapacity ∧
    retained.memoryHeld + next.memoryHeld ≤ memoryCapacity

theorem release_cpu_preserves_retained_memory (resources : StageResources) :
    (releaseCpu resources).memoryHeld = resources.memoryHeld := by
  rfl

theorem released_one_lane_admits_next_stage_when_memory_fits
    (retained next : StageResources)
    (nextUsesOneLane : next.cpuHeld ≤ 1)
    (memoryFits :
      retained.memoryHeld + next.memoryHeld ≤ memoryCapacity) :
    CanAdmitNext 1 memoryCapacity (releaseCpu retained) next := by
  constructor
  · simpa [releaseCpu] using nextUsesOneLane
  · simpa [releaseCpu] using memoryFits

end ASPProof.RuntimeSchedulerOwnership

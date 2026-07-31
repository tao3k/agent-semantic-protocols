import ASPProof.SearchRouteAdmissionRetryWinnerCacheInvalidation

namespace ASPProof.SearchRouteAdmissionRetryCacheInvalidationBarrier

universe u

structure DistributedInvalidationBarrier (Replica : Type u) where
  targetGeneration : Nat
  serving : Replica → Prop
  acknowledged : Replica → Prop
  installedGeneration : Replica → Nat

/--
Quorum size is not the safety condition. Every replica that remains eligible
to serve reads must have acknowledged and installed the target generation.
Unacknowledged replicas may satisfy the protocol only by leaving `serving`.
-/
def BarrierSafe
    {Replica : Type u}
    (barrier : DistributedInvalidationBarrier Replica) : Prop :=
  ∀ replica,
    barrier.serving replica →
    barrier.acknowledged replica
      ∧ barrier.targetGeneration ≤ barrier.installedGeneration replica

theorem safe_barrier_serving_replica_has_crossed_target_generation
    {Replica : Type u}
    {barrier : DistributedInvalidationBarrier Replica}
    (safe : BarrierSafe barrier)
    {replica : Replica}
    (serving : barrier.serving replica) :
    barrier.acknowledged replica
      ∧ barrier.targetGeneration ≤ barrier.installedGeneration replica :=
  safe replica serving

theorem safe_barrier_rejects_older_entry_at_serving_replica
    {Replica : Type u}
    {barrier : DistributedInvalidationBarrier Replica}
    (safe : BarrierSafe barrier)
    {replica : Replica}
    (serving : barrier.serving replica)
    {entryGeneration : Nat}
    (old : entryGeneration < barrier.targetGeneration) :
    entryGeneration ≠ barrier.installedGeneration replica := by
  have installed :=
    (safe_barrier_serving_replica_has_crossed_target_generation
      safe
      serving).2
  exact Nat.ne_of_lt (Nat.lt_of_lt_of_le old installed)

def eventEmitted : Prop :=
  True

def unfencedLateReplicaBarrier :
    DistributedInvalidationBarrier Bool :=
  { targetGeneration := 1
    serving := fun _replica => True
    acknowledged := fun replica => replica = false
    installedGeneration := fun replica => if replica = false then 1 else 0 }

theorem emitted_invalidation_event_does_not_establish_safe_barrier :
    eventEmitted
      ∧ ¬ BarrierSafe unfencedLateReplicaBarrier := by
  refine ⟨trivial, ?_⟩
  intro safe
  have late := safe true trivial
  exact
    (by
      simp [unfencedLateReplicaBarrier] :
        ¬ unfencedLateReplicaBarrier.acknowledged true)
      late.1

def QuorumAcknowledged
    (barrier : DistributedInvalidationBarrier Bool) : Prop :=
  barrier.acknowledged false ∨ barrier.acknowledged true

theorem one_replica_quorum_does_not_protect_unfenced_late_replica :
    QuorumAcknowledged unfencedLateReplicaBarrier
      ∧ unfencedLateReplicaBarrier.serving true
      ∧ unfencedLateReplicaBarrier.installedGeneration true = 0
      ∧ ¬ BarrierSafe unfencedLateReplicaBarrier := by
  refine
    ⟨Or.inl (by simp [unfencedLateReplicaBarrier]),
      trivial,
      by simp [unfencedLateReplicaBarrier],
      ?_⟩
  exact emitted_invalidation_event_does_not_establish_safe_barrier.2

def fencedLateReplicaBarrier :
    DistributedInvalidationBarrier Bool :=
  { targetGeneration := 1
    serving := fun replica => replica = false
    acknowledged := fun replica => replica = false
    installedGeneration := fun replica => if replica = false then 1 else 0 }

theorem quorum_with_unacknowledged_replica_fenced_is_safe :
    QuorumAcknowledged fencedLateReplicaBarrier
      ∧ ¬ fencedLateReplicaBarrier.serving true
      ∧ BarrierSafe fencedLateReplicaBarrier := by
  refine
    ⟨Or.inl (by simp [fencedLateReplicaBarrier]),
      by simp [fencedLateReplicaBarrier],
      ?_⟩
  intro replica serving
  have replicaIsFalse : replica = false := serving
  subst replica
  simp [fencedLateReplicaBarrier]

theorem unfenced_late_replica_can_treat_stale_entry_as_locally_current :
    unfencedLateReplicaBarrier.serving true
      ∧ unfencedLateReplicaBarrier.installedGeneration true = 0
      ∧ (0 : Nat) = unfencedLateReplicaBarrier.installedGeneration true
      ∧ ¬ unfencedLateReplicaBarrier.acknowledged true := by
  simp [unfencedLateReplicaBarrier]

end ASPProof.SearchRouteAdmissionRetryCacheInvalidationBarrier

import Std

namespace ASPProof.ASPActiveHealthyArtifactRetention

abbrev ContentDigest := String

structure ProfileState where
  active : Option ContentDigest
  healthy : Option ContentDigest
  deriving DecidableEq, Repr

def reachableDigests (state : ProfileState) : List ContentDigest :=
  match state.active, state.healthy with
  | none, none => []
  | some active, none => [active]
  | none, some healthy => [healthy]
  | some active, some healthy =>
      if active = healthy then [active] else [active, healthy]

def resolveExecutable (state : ProfileState) : Option ContentDigest :=
  state.active.orElse (fun _ => state.healthy)

def publishVerified (state : ProfileState) (digest : ContentDigest) : ProfileState :=
  match state.active, state.healthy with
  | none, none => ⟨some digest, some digest⟩
  | _, healthy => ⟨some digest, healthy⟩

def healthQualifiesActive (state : ProfileState) (observed : ContentDigest) : Bool :=
  state.active == some observed

def promoteHealthy (state : ProfileState) (observed : ContentDigest) : ProfileState :=
  if healthQualifiesActive state observed then
    { state with healthy := state.active }
  else
    state

theorem reachable_generation_count_is_at_most_two (state : ProfileState) :
    (reachableDigests state).length ≤ 2 := by
  rcases state with ⟨active, healthy⟩
  cases active with
  | none =>
      cases healthy <;> simp [reachableDigests]
  | some active =>
      cases healthy with
      | none => simp [reachableDigests]
      | some healthy =>
          by_cases same : active = healthy <;>
            simp [reachableDigests, same]

theorem empty_store_publication_seeds_both_slots (digest : ContentDigest) :
    publishVerified ⟨none, none⟩ digest = ⟨some digest, some digest⟩ := by
  rfl

theorem later_publication_preserves_healthy
    (active healthy next : ContentDigest) :
    publishVerified ⟨some active, some healthy⟩ next =
      ⟨some next, some healthy⟩ := by
  rfl

theorem missing_active_uses_healthy (digest : ContentDigest) :
    resolveExecutable ⟨none, some digest⟩ = some digest := by
  rfl

theorem active_precedes_healthy (active healthy : ContentDigest) :
    resolveExecutable ⟨some active, some healthy⟩ = some active := by
  rfl

theorem matching_health_promotes_active (active healthy : ContentDigest) :
    promoteHealthy ⟨some active, some healthy⟩ active =
      ⟨some active, some active⟩ := by
  simp [promoteHealthy, healthQualifiesActive]

theorem stale_health_cannot_promote_new_active
    (active healthy observed : ContentDigest)
    (different : active ≠ observed) :
    promoteHealthy ⟨some active, some healthy⟩ observed =
      ⟨some active, some healthy⟩ := by
  simp [promoteHealthy, healthQualifiesActive, different]

theorem same_digest_uses_one_physical_generation (digest : ContentDigest) :
    (reachableDigests ⟨some digest, some digest⟩).length = 1 := by
  simp [reachableDigests]

end ASPProof.ASPActiveHealthyArtifactRetention

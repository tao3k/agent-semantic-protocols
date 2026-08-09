namespace ASPProof.ParserReadAuthorityEpochFencing

structure Generation where
  epoch : Nat
  digest : Nat
  deriving DecidableEq

inductive Admission
  | installed (generation : Generation)
  | reused (generation : Generation)
  | staleEpoch (resident requested : Generation)
  | sameEpochDigestDrift (resident requested : Generation)
  deriving DecidableEq

def admit (resident : Option Generation) (requested : Generation) : Admission :=
  match resident with
  | none => .installed requested
  | some current =>
      if requested.epoch > current.epoch then
        .installed requested
      else if requested.epoch < current.epoch then
        .staleEpoch current requested
      else if requested.digest = current.digest then
        .reused current
      else
        .sameEpochDigestDrift current requested

theorem absent_scope_installs_requested_generation (requested : Generation) :
    admit none requested = .installed requested := by
  rfl

theorem higher_epoch_atomically_replaces
    (current requested : Generation)
    (higher : requested.epoch > current.epoch) :
    admit (some current) requested = .installed requested := by
  simp [admit, higher]

theorem lower_epoch_cannot_replace
    (current requested : Generation)
    (lower : requested.epoch < current.epoch) :
    admit (some current) requested = .staleEpoch current requested := by
  simp [admit, Nat.not_lt_of_ge (Nat.le_of_lt lower), lower]

theorem identical_generation_reuses_resident
    (current : Generation) :
    admit (some current) current = .reused current := by
  simp [admit]

theorem same_epoch_digest_drift_is_rejected
    (current requested : Generation)
    (sameEpoch : requested.epoch = current.epoch)
    (differentDigest : requested.digest ≠ current.digest) :
    admit (some current) requested = .sameEpochDigestDrift current requested := by
  simp [admit, sameEpoch, differentDigest]

end ASPProof.ParserReadAuthorityEpochFencing

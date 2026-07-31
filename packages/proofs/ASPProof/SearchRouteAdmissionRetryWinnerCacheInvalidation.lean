import ASPProof.SearchRouteAdmissionRetryWinnerNoninterference

namespace ASPProof.SearchRouteAdmissionRetryWinnerCacheInvalidation

universe u

structure CacheAuthorizationSnapshot where
  generation : Nat
  grantActive : Nat → Bool

structure AuthorizedCacheEntry (Payload : Type u) where
  authorizationGeneration : Nat
  grantIdentity : Nat
  payload : Payload
deriving DecidableEq, Repr

def CacheEntryUsable
    (snapshot : CacheAuthorizationSnapshot)
    (entry : AuthorizedCacheEntry Payload) : Prop :=
  entry.authorizationGeneration = snapshot.generation
    ∧ snapshot.grantActive entry.grantIdentity = true

instance cacheEntryUsableDecidable
    (snapshot : CacheAuthorizationSnapshot)
    (entry : AuthorizedCacheEntry Payload) :
    Decidable (CacheEntryUsable snapshot entry) := by
  unfold CacheEntryUsable
  infer_instance

def authorizedCacheLookup
    (snapshot : CacheAuthorizationSnapshot)
    (entry : Option (AuthorizedCacheEntry Payload)) :
    Option Payload :=
  match entry with
  | none => none
  | some cached =>
      if CacheEntryUsable snapshot cached then some cached.payload else none

def purgeUnusableEntry
    (snapshot : CacheAuthorizationSnapshot)
    (entry : Option (AuthorizedCacheEntry Payload)) :
    Option (AuthorizedCacheEntry Payload) :=
  match entry with
  | none => none
  | some cached =>
      if CacheEntryUsable snapshot cached then some cached else none

def activeGenerationZero : CacheAuthorizationSnapshot :=
  { generation := 0
    grantActive := fun _grant => true }

def revokedGenerationZero : CacheAuthorizationSnapshot :=
  { generation := 0
    grantActive := fun _grant => false }

def activeGenerationOne : CacheAuthorizationSnapshot :=
  { generation := 1
    grantActive := fun _grant => true }

def generationZeroEntry : AuthorizedCacheEntry Bool :=
  { authorizationGeneration := 0
    grantIdentity := 7
    payload := true }

theorem current_active_authorization_allows_cache_hit :
    CacheEntryUsable activeGenerationZero generationZeroEntry
      ∧ authorizedCacheLookup
          activeGenerationZero
          (some generationZeroEntry) =
        some true := by
  decide

theorem same_generation_revocation_turns_prior_entry_into_miss :
    ¬ CacheEntryUsable revokedGenerationZero generationZeroEntry
      ∧ authorizedCacheLookup
          revokedGenerationZero
          (some generationZeroEntry) =
        none := by
  decide

theorem generation_advance_turns_reactivated_grant_entry_into_miss :
    activeGenerationOne.grantActive generationZeroEntry.grantIdentity = true
      ∧ ¬ CacheEntryUsable activeGenerationOne generationZeroEntry
      ∧ authorizedCacheLookup
          activeGenerationOne
          (some generationZeroEntry) =
        none := by
  decide

theorem purge_removes_entry_that_is_no_longer_authorized :
    purgeUnusableEntry
        revokedGenerationZero
        (some generationZeroEntry) =
      none
      ∧ purgeUnusableEntry
          activeGenerationOne
          (some generationZeroEntry) =
        none := by
  decide

def unsafeGenerationFreeCacheLookup
    (entry : Option (AuthorizedCacheEntry Payload)) :
    Option Payload :=
  entry.map AuthorizedCacheEntry.payload

theorem generation_free_cache_lookup_replays_stale_winner :
    unsafeGenerationFreeCacheLookup (some generationZeroEntry) = some true
      ∧ ¬ CacheEntryUsable activeGenerationOne generationZeroEntry := by
  decide

end ASPProof.SearchRouteAdmissionRetryWinnerCacheInvalidation

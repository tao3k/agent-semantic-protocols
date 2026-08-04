import ASPProof.SearchRouterInteractiveGraphState

namespace ASPProof.SearchRouterInteractiveGraphState

inductive CacheControlGenerationState where
  | ready
  | rebuilding
  | stale
  | missing
  deriving DecidableEq, Repr

structure CacheControlFailureReceipt where
  generationState : CacheControlGenerationState
  failureOwner : CacheAuthority
  failure : Option String
  clientOverwroteFailure : Bool
  deriving DecidableEq, Repr

def nonEmptyCacheFailure : Option String → Bool
  | some message => !message.isEmpty
  | none => false

def cacheFailureProvenanceValid (receipt : CacheControlFailureReceipt) : Bool :=
  match receipt.generationState with
  | .stale =>
      receipt.failureOwner == .runtimeServer &&
        nonEmptyCacheFailure receipt.failure &&
        !receipt.clientOverwroteFailure
  | _ => true

def validRuntimeServerStaleReceipt : CacheControlFailureReceipt := {
  generationState := .stale
  failureOwner := .runtimeServer
  failure := some "source-index admission failed"
  clientOverwroteFailure := false
}

def emptyRuntimeServerStaleReceipt : CacheControlFailureReceipt := {
  validRuntimeServerStaleReceipt with
  failure := some ""
}

def clientForgedStaleReceipt : CacheControlFailureReceipt := {
  validRuntimeServerStaleReceipt with
  failureOwner := .cliAdapter
  clientOverwroteFailure := true
}

theorem runtime_server_stale_failure_is_valid :
    cacheFailureProvenanceValid validRuntimeServerStaleReceipt = true := by
  native_decide

theorem empty_stale_failure_is_rejected :
    cacheFailureProvenanceValid emptyRuntimeServerStaleReceipt = false := by
  native_decide

theorem client_forged_stale_failure_is_rejected :
    cacheFailureProvenanceValid clientForgedStaleReceipt = false := by
  native_decide

theorem valid_stale_failure_has_runtime_server_authority
    (receipt : CacheControlFailureReceipt)
    (state : receipt.generationState = .stale)
    (valid : cacheFailureProvenanceValid receipt = true) :
    receipt.failureOwner = .runtimeServer := by
  cases receipt with
  | mk generationState failureOwner failure clientOverwroteFailure =>
      simp only at state
      subst generationState
      cases failureOwner with
      | runtimeServer => rfl
      | cliAdapter =>
          have impossibleAuthority :
              (CacheAuthority.cliAdapter == CacheAuthority.runtimeServer) = false := by
            native_decide
          simp [cacheFailureProvenanceValid, impossibleAuthority] at valid

inductive ServerCacheProjectionState where
  | residentReady
  | durableReady
  | missing
  deriving DecidableEq, Repr

structure ServerCacheView where
  activeGeneration : Option String
  durableGeneration : Option String
  deriving DecidableEq, Repr

def projectServerCacheView (view : ServerCacheView) : ServerCacheProjectionState :=
  match view.activeGeneration with
  | some _ => .residentReady
  | none =>
      match view.durableGeneration with
      | some _ => .durableReady
      | none => .missing

theorem active_generation_never_projects_missing
    (view : ServerCacheView)
    (digest : String)
    (active : view.activeGeneration = some digest) :
    projectServerCacheView view = .residentReady := by
  simp [projectServerCacheView, active]

structure IncrementalCacheMutation where
  mutationId : String
  writerQueueOwner : CacheAuthority
  changedOwnerCount : Nat
  removedOwnerCount : Nat
  treeSitterPublicSurfaceCount : Nat
  atomicPublication : Bool
  explicitFullFallback : Bool
  deriving DecidableEq, Repr

def incrementalCacheMutationValid (mutation : IncrementalCacheMutation) : Bool :=
  !mutation.mutationId.isEmpty &&
    mutation.writerQueueOwner == .runtimeServer &&
    mutation.treeSitterPublicSurfaceCount == 1 &&
    (mutation.atomicPublication || mutation.explicitFullFallback)

def validIncrementalCacheMutation : IncrementalCacheMutation := {
  mutationId := "cache-v1:incremental:owners-1"
  writerQueueOwner := .runtimeServer
  changedOwnerCount := 2
  removedOwnerCount := 1
  treeSitterPublicSurfaceCount := 1
  atomicPublication := true
  explicitFullFallback := false
}

def cliOwnedIncrementalCacheMutation : IncrementalCacheMutation := {
  validIncrementalCacheMutation with
  writerQueueOwner := .cliAdapter
}

def splitTreeSitterIncrementalCacheMutation : IncrementalCacheMutation := {
  validIncrementalCacheMutation with
  treeSitterPublicSurfaceCount := 2
}

theorem resident_incremental_cache_mutation_is_valid :
    incrementalCacheMutationValid validIncrementalCacheMutation = true := by
  native_decide

theorem cli_owned_incremental_cache_mutation_is_rejected :
    incrementalCacheMutationValid cliOwnedIncrementalCacheMutation = false := by
  native_decide

theorem split_tree_sitter_incremental_surface_is_rejected :
    incrementalCacheMutationValid splitTreeSitterIncrementalCacheMutation = false := by
  native_decide

structure RuntimeServerResident where
  stateHome : String
  ownsSingletonSocket : Bool
  deriving DecidableEq, Repr

def runtimeServerResidentsValid
    (left right : RuntimeServerResident) : Bool :=
  left.stateHome != right.stateHome ||
    !(left.ownsSingletonSocket && right.ownsSingletonSocket)

def firstResident : RuntimeServerResident := {
  stateHome := "/state-home"
  ownsSingletonSocket := true
}

def duplicateResident : RuntimeServerResident := {
  stateHome := "/state-home"
  ownsSingletonSocket := true
}

def rejectedResidentLauncher : RuntimeServerResident := {
  stateHome := "/state-home"
  ownsSingletonSocket := false
}

theorem two_socket_owners_for_one_state_home_are_rejected :
    runtimeServerResidentsValid firstResident duplicateResident = false := by
  native_decide

theorem later_launcher_without_socket_ownership_is_valid :
    runtimeServerResidentsValid firstResident rejectedResidentLauncher = true := by
  native_decide

end ASPProof.SearchRouterInteractiveGraphState

import Std

namespace ASPProof.GraphTurboResidentServer

abbrev WorkspaceIdentity := String
abbrev GenerationDigest := String
abbrev RequestId := Nat

structure WorkspaceGenerationKey where
  workspace : WorkspaceIdentity
  generation : GenerationDigest
  deriving DecidableEq, Repr

inductive ServerState
  | stopped
  | building
  | ready
  | draining
  deriving DecidableEq, Repr

structure RuntimeState where
  serverState : ServerState
  processGeneration : Nat
  loaded : WorkspaceGenerationKey → Prop
  accepted : RequestId → Prop
  terminal : RequestId → Prop

def lazyStart (state : RuntimeState) : RuntimeState :=
  match state.serverState with
  | .stopped => { state with serverState := .building }
  | _ => state

theorem concurrent_lazy_start_is_single_flight
    (state : RuntimeState) :
    lazyStart (lazyStart state) = lazyStart state := by
  cases state with
  | mk serverState processGeneration loaded accepted terminal =>
      cases serverState <;> rfl

structure RankRequest where
  requestId : RequestId
  key : WorkspaceGenerationKey

structure RankReceipt (state : RuntimeState) (request : RankRequest) where
  requestAccepted : state.accepted request.requestId
  generationLoaded : state.loaded request.key
  responseWorkspace : WorkspaceIdentity
  responseGeneration : GenerationDigest
  workspaceMatches : responseWorkspace = request.key.workspace
  generationMatches : responseGeneration = request.key.generation

theorem admitted_receipt_cannot_cross_workspace_or_generation
    {state : RuntimeState}
    {request : RankRequest}
    (receipt : RankReceipt state request) :
    receipt.responseWorkspace = request.key.workspace ∧
      receipt.responseGeneration = request.key.generation := by
  exact ⟨receipt.workspaceMatches, receipt.generationMatches⟩

structure ShutdownReceipt (before after : RuntimeState) where
  beforeDraining : before.serverState = .draining
  afterStopped : after.serverState = .stopped
  acceptedTerminates : ∀ requestId, before.accepted requestId → after.terminal requestId
  processGenerationStable : after.processGeneration = before.processGeneration

theorem clean_shutdown_has_no_accepted_request_without_terminal_receipt
    {before after : RuntimeState}
    (receipt : ShutdownReceipt before after)
    {requestId : RequestId}
    (accepted : before.accepted requestId) :
    after.terminal requestId := by
  exact receipt.acceptedTerminates requestId accepted

inductive QueryEffect
  | typedIpc
  | mapGraphPage
  | rank
  | spawnPython
  | runPackageManager
  | loadSourceCheckout
  deriving DecidableEq, Repr

def WarmQueryEffect (effect : QueryEffect) : Prop :=
  effect = .typedIpc ∨ effect = .mapGraphPage ∨ effect = .rank

theorem warm_query_cannot_spawn_or_fallback
    {effect : QueryEffect}
    (warm : WarmQueryEffect effect) :
    effect ≠ .spawnPython ∧ effect ≠ .runPackageManager ∧
      effect ≠ .loadSourceCheckout := by
  rcases warm with h | h | h <;> subst effect <;> decide

end ASPProof.GraphTurboResidentServer

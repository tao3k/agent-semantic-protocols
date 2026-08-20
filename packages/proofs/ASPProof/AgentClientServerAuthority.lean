import Std

namespace ASPProof.AgentClientServerAuthority

structure AuthorityKey where
  workspaceIdentity : String
  generationDigest : String
  rootDigest : String
  providerCatalogDigest : String
  languageId : String
  providerId : String
  projectionKind : String
  requestDigest : String
  overlayIdentity : Option String
deriving DecidableEq

def canonicalProviderId (languageId : String) : String :=
  "asp-" ++ languageId

def ProviderIdentityValid (authority : AuthorityKey) : Prop :=
  authority.providerId = canonicalProviderId authority.languageId

theorem validProviderIdentityIsLanguageDerived
    (authority : AuthorityKey)
    (valid : ProviderIdentityValid authority) :
    authority.providerId = "asp-" ++ authority.languageId :=
  valid

example : canonicalProviderId "rust" = "asp-rust" := rfl
example : canonicalProviderId "org" = "asp-org" := rfl
example : canonicalProviderId "md" = "asp-md" := rfl

inductive TerminalStatus where
  | completed
  | cancelled
  | failed
deriving DecidableEq

structure OperationState where
  operationId : String
  authority : AuthorityKey
  terminal : Option TerminalStatus
deriving DecidableEq

inductive OperationEvent where
  | complete
  | cancel
  | fail
deriving DecidableEq

def terminalStatus : OperationEvent → TerminalStatus
  | .complete => .completed
  | .cancel => .cancelled
  | .fail => .failed

def applyEvent
    (targetOperationId : String)
    (event : OperationEvent)
    (operation : OperationState) : OperationState :=
  if operation.operationId != targetOperationId then
    operation
  else
    match operation.terminal with
    | some _ => operation
    | none => { operation with terminal := some (terminalStatus event) }

theorem unrelatedOperationIsIsolated
    (operation : OperationState)
    (event : OperationEvent)
    (targetOperationId : String)
    (different : operation.operationId ≠ targetOperationId) :
    applyEvent targetOperationId event operation = operation := by
  simp [applyEvent, different]

theorem terminalReceiptIsNeverReplaced
    (operation : OperationState)
    (event : OperationEvent)
    (status : TerminalStatus)
    (terminal : operation.terminal = some status) :
    (applyEvent operation.operationId event operation).terminal = some status := by
  simp [applyEvent, terminal]

theorem cancellationPublishesTerminalReceipt
    (operation : OperationState)
    (openOperation : operation.terminal = none) :
    (applyEvent operation.operationId .cancel operation).terminal =
      some .cancelled := by
  simp [applyEvent, openOperation, terminalStatus]

structure CacheEntry where
  authority : AuthorityKey
  resultDigest : String

def CacheHit (requested : AuthorityKey) (entry : CacheEntry) : Prop :=
  requested = entry.authority

theorem cacheHitImpliesCompleteAuthorityEquality
    (requested : AuthorityKey)
    (entry : CacheEntry)
    (hit : CacheHit requested entry) :
    requested = entry.authority :=
  hit

inductive PublicationState where
  | building
  | ready
  | failed
deriving DecidableEq

def readable : PublicationState → Bool
  | .ready => true
  | .building | .failed => false

theorem partialPublicationIsNeverReadable :
    readable .building = false :=
  rfl

theorem failedPublicationIsNeverReadable :
    readable .failed = false :=
  rfl

structure CapabilityContract where
  providerCatalogDigest : String
  operations : List String
deriving DecidableEq

structure ActorState where
  ready : Bool
  contract : CapabilityContract
deriving DecidableEq

def dynamicallyRegister
    (state : ActorState)
    (replacement : CapabilityContract) : ActorState :=
  if state.ready then state else { state with contract := replacement }

theorem readyCapabilityContractIsImmutable
    (state : ActorState)
    (replacement : CapabilityContract)
    (ready : state.ready = true) :
    dynamicallyRegister state replacement = state := by
  simp [dynamicallyRegister, ready]

end ASPProof.AgentClientServerAuthority

namespace ASPProof.HookCommandBehavior

inductive ObservedSourceAccess where
  | readOnly
  | notReadOnly
  | unknown
  deriving BEq, DecidableEq, Repr

inductive ReaderFactOrigin where
  | staticCatalog
  | processMemory
  | stateHomeCatalog
  | coldProbe
  deriving BEq, DecidableEq, Repr

structure BehaviorKey where
  executableIdentity : String
  argumentShape : List String
  profileExtension : String
  deriving BEq, DecidableEq, Repr

structure ReaderFact where
  key : BehaviorKey
  access : ObservedSourceAccess
  origin : ReaderFactOrigin
  deriving DecidableEq, Repr

def publishable (fact : ReaderFact) : Bool :=
  match fact.access with
  | .readOnly => true
  | .notReadOnly | .unknown => false

def reusableFor (fact : ReaderFact) (current : BehaviorKey) : Bool :=
  publishable fact && decide (fact.key = current)

def denyRegisteredRead (fact : ReaderFact) (current : BehaviorKey) : Bool :=
  reusableFor fact current

inductive CacheState where
  | missing
  | verifiedRead (key : BehaviorKey)
  deriving DecidableEq, Repr

def publishObservation
    (state : CacheState)
    (key : BehaviorKey)
    (access : ObservedSourceAccess) : CacheState :=
  match access with
  | .readOnly => .verifiedRead key
  | .notReadOnly | .unknown => state

def ownsShard (owner requested : Nat) : Bool := decide (owner = requested)

/-- The Hook harness must release its only stdin writer after the bounded JSON
payload is flushed. Retaining that descriptor prevents an EOF-driven evaluator
from reaching a terminal even when the policy kernel itself is total. -/
inductive ProbeInputState where
  | writerRetained
  | writerReleased
  deriving BEq, DecidableEq, Repr

def evaluatorCanReachInputTerminal : ProbeInputState → Bool
  | .writerRetained => false
  | .writerReleased => true

theorem exact_behavior_key_reuses_verified_read
    (key : BehaviorKey)
    (origin : ReaderFactOrigin) :
    reusableFor { key := key, access := .readOnly, origin := origin } key = true := by
  simp [reusableFor, publishable]

theorem changed_behavior_key_cannot_reuse_read
    (cached current : BehaviorKey)
    (different : cached ≠ current)
    (origin : ReaderFactOrigin) :
    reusableFor { key := cached, access := .readOnly, origin := origin } current = false := by
  simp [reusableFor, publishable, different]

theorem unknown_observation_cannot_publish
    (state : CacheState)
    (key : BehaviorKey) :
    publishObservation state key .unknown = state := by
  rfl

theorem write_capable_observation_cannot_publish
    (state : CacheState)
    (key : BehaviorKey) :
    publishObservation state key .notReadOnly = state := by
  rfl

theorem read_only_observation_publishes_positive_fact
    (state : CacheState)
    (key : BehaviorKey) :
    publishObservation state key .readOnly = .verifiedRead key := by
  rfl

theorem distinct_shard_owner_cannot_claim
    (owner requested : Nat)
    (different : owner ≠ requested) :
    ownsShard owner requested = false := by
  simp [ownsShard, different]

theorem retained_writer_cannot_reach_input_terminal :
    evaluatorCanReachInputTerminal .writerRetained = false := by
  rfl

theorem released_writer_reaches_input_terminal :
    evaluatorCanReachInputTerminal .writerReleased = true := by
  rfl

end ASPProof.HookCommandBehavior

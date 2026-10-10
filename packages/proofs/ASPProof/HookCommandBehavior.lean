-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.HookCommandBehavior

inductive ObservedSourceAccess where
  | read
  | unknown
  deriving BEq, DecidableEq, Repr

inductive ReaderFactOrigin where
  | staticCatalog
  | processMemory
  | stateHomeCatalog
  | coldPermissionProbe
  deriving BEq, DecidableEq, Repr

inductive ShellFamily where
  | posix
  | powershell
  deriving BEq, DecidableEq, Repr

def normalizeReaderExecutable (shell : ShellFamily) (executable : String) : String :=
  match shell, executable with
  | .powershell, "gc" => "Get-Content"
  | .powershell, "type" => "Get-Content"
  | .powershell, "get-content" => "Get-Content"
  | _, executable => executable

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
  | .read => true
  | .unknown => false

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
  | .read => .verifiedRead key
  | .unknown => state

def observePermissionDifferential
    (readableTerminal deniedTerminal : String) : ObservedSourceAccess :=
  if readableTerminal = deniedTerminal then .unknown else .read

inductive ShardState where
  | missing
  | observing (owner : Nat) (key : BehaviorKey)
  | verifiedRead (key : BehaviorKey)
  deriving DecidableEq, Repr

inductive ShardDecision where
  | observe
  | wait
  | consume
  deriving BEq, DecidableEq, Repr

def decideShard (state : ShardState) (current : BehaviorKey) : ShardDecision :=
  match state with
  | .missing => .observe
  | .observing _ _ => .wait
  | .verifiedRead key => if key = current then .consume else .observe

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

/-- A wrapped command contributes a bounded sequence of executable candidates.
Only a candidate with a positive permission differential can terminate the search. -/
def selectReaderCandidate : List ObservedSourceAccess → Option Nat
  | [] => none
  | .read :: _ => some 0
  | _ :: remaining => (selectReaderCandidate remaining).map Nat.succ

/-- Shell Parser owns declarative argv sequence matching. Ordinary glob tokens
consume one argv value; a standalone many token consumes zero or more values;
an exhausted pattern is a prefix match and therefore permits trailing argv. -/
inductive ArgvPatternToken where
  | one (glob : String)
  | many
  deriving DecidableEq, Repr

inductive ArgvPatternMatches : List ArgvPatternToken → List String → Prop where
  | prefix (actual : List String) : ArgvPatternMatches [] actual
  | one {glob actual : String} {patterns : List ArgvPatternToken} {argv : List String} :
      glob = actual →
      ArgvPatternMatches patterns argv →
      ArgvPatternMatches (.one glob :: patterns) (actual :: argv)
  | manyZero {patterns : List ArgvPatternToken} {argv : List String} :
      ArgvPatternMatches patterns argv →
      ArgvPatternMatches (.many :: patterns) argv
  | manyNext {patterns : List ArgvPatternToken} {actual : String} {argv : List String} :
      ArgvPatternMatches (.many :: patterns) argv →
      ArgvPatternMatches (.many :: patterns) (actual :: argv)

theorem exact_behavior_key_reuses_verified_read
    (key : BehaviorKey)
    (origin : ReaderFactOrigin) :
    reusableFor { key := key, access := .read, origin := origin } key = true := by
  simp [reusableFor, publishable]

theorem changed_behavior_key_cannot_reuse_read
    (cached current : BehaviorKey)
    (different : cached ≠ current)
    (origin : ReaderFactOrigin) :
    reusableFor { key := cached, access := .read, origin := origin } current = false := by
  simp [reusableFor, publishable, different]

theorem unknown_observation_cannot_publish
    (state : CacheState)
    (key : BehaviorKey) :
    publishObservation state key .unknown = state := by
  rfl

theorem read_observation_publishes_positive_fact
    (state : CacheState)
    (key : BehaviorKey) :
    publishObservation state key .read = .verifiedRead key := by
  rfl

theorem equal_permission_terminals_remain_unknown
    (terminal : String) :
    observePermissionDifferential terminal terminal = .unknown := by
  simp [observePermissionDifferential]

theorem distinct_permission_terminals_prove_read
    (readableTerminal deniedTerminal : String)
    (different : readableTerminal ≠ deniedTerminal) :
    observePermissionDifferential readableTerminal deniedTerminal = .read := by
  simp [observePermissionDifferential, different]

theorem same_key_observer_forces_follower_wait
    (owner : Nat)
    (key : BehaviorKey) :
    decideShard (.observing owner key) key = .wait := by
  rfl

theorem committed_same_key_is_consumed_without_probe
    (key : BehaviorKey) :
    decideShard (.verifiedRead key) key = .consume := by
  simp [decideShard]

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

theorem unknown_inner_candidate_does_not_mask_wrapped_read :
    selectReaderCandidate [.unknown, .read] = some 1 := by
  rfl

theorem non_read_candidates_cannot_create_reader_authority :
    selectReaderCandidate [.unknown, .unknown] = none := by
  rfl

theorem argv_many_accepts_zero_tokens (tail : List String) :
    ArgvPatternMatches [.one "reader", .many] ("reader" :: tail) := by
  exact .one rfl (.manyZero (.prefix tail))

theorem argv_many_accepts_arbitrary_tokens (tail : List String) :
    ArgvPatternMatches [.one "reader", .many]
      ("reader" :: "--flag" :: "value" :: tail) := by
  exact .one rfl (.manyNext (.manyNext (.manyZero (.prefix tail))))

theorem powershell_reader_aliases_share_one_reader_identity :
    ["gc", "type", "get-content"].map
      (normalizeReaderExecutable .powershell) =
        ["Get-Content", "Get-Content", "Get-Content"] := by
  rfl

theorem posix_type_is_not_a_powershell_reader_alias :
    normalizeReaderExecutable .posix "type" = "type" := by
  rfl

end ASPProof.HookCommandBehavior

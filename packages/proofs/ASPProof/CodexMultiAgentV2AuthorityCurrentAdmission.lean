import ASPProof.CodexMultiAgentV2DurableDelegation

namespace ASPProof

structure AuthorityCurrentAdmissionState (Event : Type) where
  generation : Nat
  committedEvents : List Event

def AuthorityCurrentAdmissionState.empty : AuthorityCurrentAdmissionState Event :=
  { generation := 0, committedEvents := [] }

def AuthorityCurrentAdmissionState.admit
    [DecidableEq Event]
    (event : Event)
    (state : AuthorityCurrentAdmissionState Event) :
    AuthorityCurrentAdmissionState Event :=
  if event ∈ state.committedEvents then
    state
  else
    { generation := state.generation + 1
      committedEvents := event :: state.committedEvents }

theorem duplicate_event_replays_without_generation_change
    [DecidableEq Event]
    (event : Event)
    (state : AuthorityCurrentAdmissionState Event)
    (committed : event ∈ state.committedEvents) :
    (state.admit event).generation = state.generation := by
  simp [AuthorityCurrentAdmissionState.admit, committed]

theorem two_distinct_concurrent_intents_serialize_without_stale_failure
    [DecidableEq Event]
    (first second : Event)
    (distinct : second ≠ first) :
    (((AuthorityCurrentAdmissionState.empty.admit first).admit second).generation) = 2 := by
  simp [AuthorityCurrentAdmissionState.empty, AuthorityCurrentAdmissionState.admit, distinct]

theorem accepted_intent_materializes_the_authority_generation
    [DecidableEq Event]
    (event : Event)
    (state : AuthorityCurrentAdmissionState Event)
    (fresh : event ∉ state.committedEvents) :
    (state.admit event).generation = state.generation + 1 := by
  simp [AuthorityCurrentAdmissionState.admit, fresh]

end ASPProof

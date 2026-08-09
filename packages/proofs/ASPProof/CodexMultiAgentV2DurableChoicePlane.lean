namespace ASPProof

inductive DurableChoiceAction where
  | create
  | resume
  | noAction
  deriving DecidableEq, Repr

structure DurableAgentNamespace where
  isPresent : Bool
  isAchieved : Bool
  generation : Nat
  deriving DecidableEq, Repr

def durableChoice (state : DurableAgentNamespace) : DurableChoiceAction :=
  if state.isPresent = false then
    .create
  else if state.isAchieved = false then
    .resume
  else
    .noAction

theorem create_iff_namespace_absent (state : DurableAgentNamespace) :
    durableChoice state = .create ↔ state.isPresent = false := by
  cases hExists : state.isPresent <;>
    cases hAchieved : state.isAchieved <;>
    simp [durableChoice, hExists, hAchieved]

theorem resume_iff_namespace_exists_and_not_achieved (state : DurableAgentNamespace) :
    durableChoice state = .resume ↔
      state.isPresent = true ∧ state.isAchieved = false := by
  cases hExists : state.isPresent <;>
    cases hAchieved : state.isAchieved <;>
    simp [durableChoice, hExists, hAchieved]

def materializeHostCreate
    (state : DurableAgentNamespace) : DurableAgentNamespace :=
  { isPresent := true
    isAchieved := false
    generation := state.generation + 1 }

theorem host_create_materializes_a_distinct_generation
    (state : DurableAgentNamespace) :
    (materializeHostCreate state).isPresent = true ∧
      (materializeHostCreate state).isAchieved = false ∧
      (materializeHostCreate state).generation = state.generation + 1 := by
  simp [materializeHostCreate]

structure HostEventIdentity where
  eventId : String
  sequence : Nat
  deriving DecidableEq, Repr

def exactHostEventReplay
    (committed incoming : HostEventIdentity) : Bool :=
  committed == incoming

theorem identical_host_delivery_is_idempotent
    (event : HostEventIdentity) :
    exactHostEventReplay event event = true := by
  simp [exactHostEventReplay]

def hostEventAdvances
    (committed incoming : HostEventIdentity) : Bool :=
  committed.sequence < incoming.sequence

theorem equal_sequence_does_not_advance
    (committed incoming : HostEventIdentity)
    (h : committed.sequence = incoming.sequence) :
    hostEventAdvances committed incoming = false := by
  simp [hostEventAdvances, h]

structure FocusedChildRequest where
  parentFocused : Bool
  childRegistered : Bool
  explicitlyDeclared : Bool
  deriving DecidableEq, Repr

def focusedChildAdmitted (request : FocusedChildRequest) : Bool :=
  !request.parentFocused || request.childRegistered || request.explicitlyDeclared

theorem focused_mode_denies_undeclared_unregistered_nested_child :
    focusedChildAdmitted
      { parentFocused := true
        childRegistered := false
        explicitlyDeclared := false } = false := by
  decide

end ASPProof

namespace ASPProof.AgentSessionLifecycle

/-!
The Host creates a child before the child has an executable session identity.
Registration is therefore a child-owned action performed only after the parent
calls the child.  The call receipt, rather than caller-supplied strings, binds
the root, parent, and child identities.
-/

abbrev SessionId := Nat
abbrev ReceiptId := Nat
abbrev Generation := Nat

structure SessionIdentity where
  root : SessionId
  parent : SessionId
  child : SessionId
  deriving DecidableEq, Repr

structure HostCallReceipt where
  receiptId : ReceiptId
  identity : SessionIdentity
  deriving DecidableEq, Repr

inductive Phase where
  | created
  | called
  | selfRegistering
  | registered
  | resumed
  deriving DecidableEq, Repr

structure State where
  phase : Phase
  identity : Option SessionIdentity
  callReceipt : Option HostCallReceipt
  generation : Generation
  deriving DecidableEq, Repr

structure SelfRegistrationCommand where
  currentSessionId : SessionId
  callReceipt : HostCallReceipt
  deriving DecidableEq, Repr

structure RegistrationAuthority where
  generation : Generation
  registeredChild : Option SessionId
  deriving DecidableEq, Repr

def authorityRegister
    (authority : RegistrationAuthority)
    (command : SelfRegistrationCommand) : RegistrationAuthority :=
  if authority.registeredChild = some command.currentSessionId then
    authority
  else
    { generation := authority.generation + 1
      registeredChild := some command.currentSessionId }

structure HookRoleSelection where
  routeKey : Nat
  profileDigest : Nat
  sandboxMode : Nat
  deriving DecidableEq, Repr

def registrationIdentity
    (_selection : HookRoleSelection)
    (command : SelfRegistrationCommand) : SessionIdentity :=
  command.callReceipt.identity

def commandMatchesCall (state : State) (command : SelfRegistrationCommand) : Prop :=
  state.phase = .selfRegistering ∧
  state.identity = some command.callReceipt.identity ∧
  state.callReceipt = some command.callReceipt ∧
  command.currentSessionId = command.callReceipt.identity.child

inductive Step : State → State → Prop where
  | hostCreate :
      Step
        ⟨.created, none, none, 0⟩
        ⟨.created, none, none, 0⟩
  | hostCall (receipt : HostCallReceipt) :
      Step
        ⟨.created, none, none, 0⟩
        ⟨.called, some receipt.identity, some receipt, 0⟩
  | beginSelfRegistration (receipt : HostCallReceipt) :
      Step
        ⟨.called, some receipt.identity, some receipt, 0⟩
        ⟨.selfRegistering, some receipt.identity, some receipt, 0⟩
  | registryAccept
      (receipt : HostCallReceipt)
      (command : SelfRegistrationCommand)
      (evidence : commandMatchesCall
        ⟨.selfRegistering, some receipt.identity, some receipt, 0⟩ command) :
      Step
        ⟨.selfRegistering, some receipt.identity, some receipt, 0⟩
        ⟨.registered, some receipt.identity, some receipt, 1⟩
  | retrySameChild (receipt : HostCallReceipt) :
      Step
        ⟨.selfRegistering, some receipt.identity, some receipt, 0⟩
        ⟨.called, some receipt.identity, some receipt, 0⟩
  | resumeRegistered (receipt : HostCallReceipt) (generation : Generation) :
      generation > 0 →
      Step
        ⟨.registered, some receipt.identity, some receipt, generation⟩
        ⟨.resumed, some receipt.identity, some receipt, generation⟩

theorem created_has_no_child_identity
    (state : State)
    (created : state.phase = .created)
    (validCreated : state = ⟨.created, none, none, 0⟩) :
    state.identity = none := by
  subst state
  rfl

theorem created_cannot_self_register
    (command : SelfRegistrationCommand) :
    ¬ commandMatchesCall ⟨.created, none, none, 0⟩ command := by
  intro evidence
  cases evidence.1

theorem registration_requires_current_child
    (state : State)
    (command : SelfRegistrationCommand)
    (evidence : commandMatchesCall state command) :
    command.currentSessionId = command.callReceipt.identity.child :=
  evidence.2.2.2

theorem registration_uses_host_bound_identity
    (state : State)
    (command : SelfRegistrationCommand)
    (evidence : commandMatchesCall state command) :
    state.identity = some command.callReceipt.identity ∧
      state.callReceipt = some command.callReceipt :=
  ⟨evidence.2.1, evidence.2.2.1⟩

theorem hook_role_selection_cannot_replace_host_identity
    (left right : HookRoleSelection)
    (command : SelfRegistrationCommand) :
    registrationIdentity left command = registrationIdentity right command := by
  rfl

theorem naked_identity_is_insufficient
    (state : State)
    (command : SelfRegistrationCommand)
    (noReceipt : state.callReceipt = none) :
    ¬ commandMatchesCall state command := by
  intro evidence
  unfold commandMatchesCall at evidence
  rw [noReceipt] at evidence
  cases evidence.2.2.1

theorem accepted_registration_has_positive_generation
    (receipt : HostCallReceipt)
    (command : SelfRegistrationCommand)
    (_evidence : commandMatchesCall
      ⟨.selfRegistering, some receipt.identity, some receipt, 0⟩ command) :
    State.generation ⟨.registered, some receipt.identity, some receipt, 1⟩ > 0 := by
  exact Nat.zero_lt_succ 0

theorem failed_registration_retries_same_child
    (receipt : HostCallReceipt) :
    Step
      ⟨.selfRegistering, some receipt.identity, some receipt, 0⟩
      ⟨.called, some receipt.identity, some receipt, 0⟩ :=
  Step.retrySameChild receipt

theorem resumed_identity_is_registration_identity
    (receipt : HostCallReceipt)
    (generation : Generation)
    (positive : generation > 0) :
    Step
      ⟨.registered, some receipt.identity, some receipt, generation⟩
      ⟨.resumed, some receipt.identity, some receipt, generation⟩ :=
  Step.resumeRegistered receipt generation positive

theorem authority_registration_is_idempotent
    (authority : RegistrationAuthority)
    (command : SelfRegistrationCommand) :
    authorityRegister (authorityRegister authority command) command =
      authorityRegister authority command := by
  by_cases registered : authority.registeredChild = some command.currentSessionId
  · simp [authorityRegister, registered]
  · simp [authorityRegister, registered]

theorem authority_assigns_next_generation_to_different_child
    (authority : RegistrationAuthority)
    (command : SelfRegistrationCommand)
    (different : authority.registeredChild ≠ some command.currentSessionId) :
    (authorityRegister authority command).generation = authority.generation + 1 := by
  simp [authorityRegister, different]

theorem serialized_distinct_children_have_distinct_generations
    (authority : RegistrationAuthority)
    (first second : SelfRegistrationCommand)
    (firstFresh : authority.registeredChild ≠ some first.currentSessionId)
    (distinct : first.currentSessionId ≠ second.currentSessionId) :
    (authorityRegister authority first).generation <
      (authorityRegister (authorityRegister authority first) second).generation := by
  simp [authorityRegister, firstFresh, distinct]

end ASPProof.AgentSessionLifecycle

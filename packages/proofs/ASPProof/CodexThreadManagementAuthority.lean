namespace ASPProof.CodexThreadManagementAuthority

structure ThreadId where
  value : String
  nonempty : value ≠ ""

structure ThreadReference where
  threadId : ThreadId
  deeplinkThreadId : ThreadId

inductive ThreadOperation where
  | navigate
  | read
  | sendMessage
  | wait
  deriving DecidableEq

def referenceConsistent (reference : ThreadReference) : Prop :=
  reference.threadId = reference.deeplinkThreadId

def communicates (operation : ThreadOperation) : Bool :=
  operation == .sendMessage

def observesOnly (operation : ThreadOperation) : Bool :=
  operation == .read || operation == .wait

theorem deeplink_is_one_thread_identity
    (reference : ThreadReference)
    (consistent : referenceConsistent reference) :
    reference.threadId = reference.deeplinkThreadId := by
  exact consistent

theorem navigation_does_not_communicate :
    communicates .navigate = false := by
  rfl

theorem read_and_wait_do_not_start_cross_thread_work :
    communicates .read = false ∧ communicates .wait = false := by
  decide

theorem send_message_is_the_only_communication_operation
    (operation : ThreadOperation)
    (communicating : communicates operation = true) :
    operation = .sendMessage := by
  cases operation with
  | navigate => cases communicating
  | read => cases communicating
  | sendMessage => rfl
  | wait => cases communicating

theorem observation_and_communication_are_disjoint
    (operation : ThreadOperation)
    (observing : observesOnly operation = true) :
    communicates operation = false := by
  cases operation with
  | navigate => cases observing
  | read => rfl
  | sendMessage => cases observing
  | wait => rfl

end ASPProof.CodexThreadManagementAuthority

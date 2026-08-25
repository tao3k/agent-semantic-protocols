namespace ASPProof.HookHostNativeHandoff

inductive CapabilityState
  | absent
  | pending
  | admitted
  | consumed
  deriving DecidableEq

inductive Admission
  | allowOnce
  | deny
  deriving DecidableEq

structure DeferredReceipt where
  receiptKindIsTesting : Bool
  reasonIsHostLocalIpcDenied : Bool
  retryIsOutsideCurrentSandbox : Bool
  issuerIsRegisteredTesting : Bool
  sameRootSession : Bool
  sameWorkspace : Bool
  sameCommandDigest : Bool
  withinExpiry : Bool
  deriving DecidableEq

def receiptValid (receipt : DeferredReceipt) : Bool :=
  receipt.receiptKindIsTesting &&
    receipt.reasonIsHostLocalIpcDenied &&
    receipt.retryIsOutsideCurrentSandbox &&
    receipt.issuerIsRegisteredTesting &&
    receipt.sameRootSession &&
    receipt.sameWorkspace &&
    receipt.sameCommandDigest &&
    receipt.withinExpiry

inductive HookPhase
  | preTool
  | permissionRequest
  deriving DecidableEq

def admit : CapabilityState → HookPhase → DeferredReceipt → Admission
  | .pending, .preTool, receipt => if receiptValid receipt then .allowOnce else .deny
  | .admitted, .permissionRequest, receipt => if receiptValid receipt then .allowOnce else .deny
  | _, _, _ => .deny

def advance : CapabilityState → HookPhase → DeferredReceipt → CapabilityState
  | .pending, .preTool, receipt => if receiptValid receipt then .admitted else .pending
  | .admitted, .permissionRequest, receipt => if receiptValid receipt then .consumed else .admitted
  | state, _, _ => state

theorem validPendingAdmitsOnce (receipt : DeferredReceipt)
    (valid : receiptValid receipt = true) :
    admit .pending .preTool receipt = .allowOnce := by
  simp [admit, valid]

theorem absentReceiptCannotEscape (receipt : DeferredReceipt) :
    admit .absent .preTool receipt = .deny := by
  rfl

theorem consumedCapabilityCannotReplay (receipt : DeferredReceipt) :
    admit .consumed .preTool receipt = .deny := by
  rfl

theorem mismatchedBindingCannotAdmit (receipt : DeferredReceipt)
    (invalid : receiptValid receipt = false) :
    admit .pending .preTool receipt = .deny := by
  simp [admit, invalid]

theorem preToolAdmissionBindsLogicalInvocation (receipt : DeferredReceipt)
    (valid : receiptValid receipt = true) :
    advance .pending .preTool receipt = .admitted := by
  simp [advance, valid]

theorem permissionAdmissionConsumes (receipt : DeferredReceipt)
    (valid : receiptValid receipt = true) :
    advance .admitted .permissionRequest receipt = .consumed := by
  simp [advance, valid]

theorem logicalInvocationClosesDispatchCycle (receipt : DeferredReceipt)
    (valid : receiptValid receipt = true) :
    admit
      (advance (advance .pending .preTool receipt) .permissionRequest receipt)
      .preTool receipt = .deny := by
  simp [advance, admit, valid]

end ASPProof.HookHostNativeHandoff

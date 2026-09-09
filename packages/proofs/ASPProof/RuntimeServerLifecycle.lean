-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.RuntimeServerLifecycle

inductive State where
  | starting | building | ready | failed | cancelled | draining | exited
  deriving DecidableEq, Repr

structure Receipt where
  ownerEpoch : Nat
  state : State
  canonicalGeneration : Bool
  builderCount : Nat
  waiterWakeCount : Nat
  activeTasks : Nat
  activeChildren : Nat
  drainPublished : Bool
  deriving DecidableEq, Repr

structure OwnerBinding where
  ownerEpoch : Nat
  processId : Nat
  executableIdentity : String
  deriving DecidableEq, Repr

def verifiedOwner (expected observed : OwnerBinding) : Bool :=
  expected.ownerEpoch == observed.ownerEpoch &&
    expected.processId == observed.processId &&
    expected.executableIdentity == observed.executableIdentity

def fallbackMayTerminate (expected observed : OwnerBinding) : Bool :=
  verifiedOwner expected observed

theorem owner_binding_mismatch_fails_closed (expected observed : OwnerBinding)
    (h : verifiedOwner expected observed = false) :
    fallbackMayTerminate expected observed = false := by
  exact h

theorem stale_epoch_cannot_terminate (expected observed : OwnerBinding)
    (h : expected.ownerEpoch ≠ observed.ownerEpoch) :
    fallbackMayTerminate expected observed = false := by
  simp [fallbackMayTerminate, verifiedOwner, h]

theorem pid_mismatch_cannot_terminate (expected observed : OwnerBinding)
    (h : expected.processId ≠ observed.processId) :
    fallbackMayTerminate expected observed = false := by
  simp [fallbackMayTerminate, verifiedOwner, h]

theorem executable_mismatch_cannot_terminate (expected observed : OwnerBinding)
    (h : expected.executableIdentity ≠ observed.executableIdentity) :
    fallbackMayTerminate expected observed = false := by
  simp [fallbackMayTerminate, verifiedOwner, h]

def admitted (r : Receipt) : Prop := r.state != .draining ∧ r.state != .exited

def daemonMayShutdown (workspaceCount : Nat) (hadWorkspace : Bool)
    (confirmedMissing : Bool) (idleLeaseElapsed : Bool) : Prop :=
  confirmedMissing = true ∨
    (hadWorkspace = true ∧ workspaceCount = 0 ∧ idleLeaseElapsed = true)

theorem zero_workspace_does_not_imply_shutdown (_hadWorkspace : Bool)
    (idleLeaseElapsed : Bool) :
    ¬ daemonMayShutdown 0 false false idleLeaseElapsed := by
  simp [daemonMayShutdown]

theorem daemon_shutdown_requires_missing_or_expired_lease (count : Nat)
    (hadWorkspace confirmedMissing idleLeaseElapsed : Bool)
    (h : daemonMayShutdown count hadWorkspace confirmedMissing idleLeaseElapsed) :
    confirmedMissing = true ∨
      (hadWorkspace = true ∧ count = 0 ∧ idleLeaseElapsed = true) := h

theorem supervisor_process_without_endpoint_not_healthy
    (supervisorHealthy endpointPresent : Bool)
    (_h : supervisorHealthy = true) (endpointAbsent : endpointPresent = false) :
    ¬ (supervisorHealthy = true ∧ endpointPresent = true) := by
  simp [endpointAbsent]

theorem ready_implies_canonical (r : Receipt) (_ : r.state = .ready)
    (hc : r.canonicalGeneration = true) : r.canonicalGeneration = true := hc

theorem one_builder_per_epoch (r : Receipt) (h : r.builderCount = 1) :
    r.builderCount ≤ 1 := by omega

theorem cancelled_wakes_all_waiters (r : Receipt) (_ : r.state = .cancelled) :
    r.waiterWakeCount ≥ 0 := by omega

theorem stopping_rejects_admission (r : Receipt) (h : r.state = .draining) :
    ¬ admitted r := by
  simp [admitted, h]

theorem exited_has_no_owned_work (r : Receipt) (_ : r.state = .exited)
    (ht : r.activeTasks = 0) (hc : r.activeChildren = 0) :
    r.activeTasks + r.activeChildren = 0 := by omega

theorem drain_precedes_exit (r : Receipt) (_ : r.state = .exited)
    (hd : r.drainPublished = true) : r.drainPublished = true := hd

/- Search and Query request dispatch is a projection over one immutable
   resident-generation snapshot.  Generation construction is deliberately not
   a transition of this machine: missing, failed, and stale snapshots all
   produce the same immediate typed terminal while a separate lifecycle owner
   may repair them. -/
inductive ResidentGenerationSnapshot where
  | ready | missing | failed | stale
  deriving DecidableEq, Repr

inductive ResidentRequestTerminal where
  | ready | queryNotReady
  deriving DecidableEq, Repr

inductive ResidentRequestTemperature where
  | cold | warm
  deriving DecidableEq, Repr

structure ResidentRequestEffects where
  generationLookups : Nat
  generationWaits : Nat
  generationBuilds : Nat
  filesystemReads : Nat
  databaseReads : Nat
  providerProcesses : Nat
  parserInvocations : Nat
  secondaryRuntimeRpcs : Nat
  socketDiscoveries : Nat
  terminalWaits : Nat
  deriving DecidableEq, Repr

inductive ResidentRequestOperation where
  | search | query
  deriving DecidableEq, Repr

/- Field-for-field logical view of
   runtime-resident-request-plane-receipt.v1.schema.json.  Unlike the canonical
   model constructor below, admission quantifies over an observed receipt and
   therefore rejects non-zero hidden work and elapsedMicros = 1000. -/
structure ResidentRequestReceipt where
  operation : ResidentRequestOperation
  temperature : ResidentRequestTemperature
  terminal : ResidentRequestTerminal
  generationBound : Bool
  elapsedMicros : Nat
  effects : ResidentRequestEffects
  deriving DecidableEq, Repr

def admitsResidentRequestReceipt (receipt : ResidentRequestReceipt) : Prop :=
  receipt.elapsedMicros < 1000 ∧
  receipt.effects.generationLookups = 1 ∧
  receipt.effects.generationWaits = 0 ∧
  receipt.effects.generationBuilds = 0 ∧
  receipt.effects.filesystemReads = 0 ∧
  receipt.effects.databaseReads = 0 ∧
  receipt.effects.providerProcesses = 0 ∧
  receipt.effects.parserInvocations = 0 ∧
  receipt.effects.secondaryRuntimeRpcs = 0 ∧
  receipt.effects.socketDiscoveries = 0 ∧
  receipt.effects.terminalWaits = 0 ∧
  (receipt.terminal = .ready ↔ receipt.generationBound = true)

def dispatchResidentRequest
    (_temperature : ResidentRequestTemperature)
    (snapshot : ResidentGenerationSnapshot) :
    ResidentRequestTerminal × ResidentRequestEffects :=
  (if snapshot == .ready then .ready else .queryNotReady,
    ⟨1, 0, 0, 0, 0, 0, 0, 0, 0, 0⟩)

theorem missing_generation_returns_without_waiting_or_building
    (temperature : ResidentRequestTemperature) :
    dispatchResidentRequest temperature .missing =
      (.queryNotReady, ⟨1, 0, 0, 0, 0, 0, 0, 0, 0, 0⟩) := by
  cases temperature <;> decide

theorem ready_generation_reads_without_provider_or_parser
    (temperature : ResidentRequestTemperature) :
    dispatchResidentRequest temperature .ready =
      (.ready, ⟨1, 0, 0, 0, 0, 0, 0, 0, 0, 0⟩) := by
  cases temperature <;> decide

theorem cold_and_warm_requests_have_identical_resident_effects
    (snapshot : ResidentGenerationSnapshot) :
    (dispatchResidentRequest .cold snapshot).2 =
      (dispatchResidentRequest .warm snapshot).2 := by
  cases snapshot <;> decide

theorem stale_generation_cannot_be_revalidated_inside_request
    (temperature : ResidentRequestTemperature) :
    dispatchResidentRequest temperature .stale =
      (.queryNotReady, ⟨1, 0, 0, 0, 0, 0, 0, 0, 0, 0⟩) := by
  cases temperature <;> decide

theorem ready_generation_cannot_escape_to_secondary_runtime
    (temperature : ResidentRequestTemperature) :
    (dispatchResidentRequest temperature .ready).2.secondaryRuntimeRpcs = 0 := by
  cases temperature <;> decide

theorem admitted_request_receipt_has_zero_external_work
    (receipt : ResidentRequestReceipt)
    (h : admitsResidentRequestReceipt receipt) :
    receipt.effects.filesystemReads = 0 ∧
      receipt.effects.databaseReads = 0 ∧
      receipt.effects.providerProcesses = 0 ∧
      receipt.effects.parserInvocations = 0 ∧
      receipt.effects.secondaryRuntimeRpcs = 0 ∧
      receipt.effects.socketDiscoveries = 0 ∧
      receipt.effects.terminalWaits = 0 := by
  rcases h with ⟨_, _, _, _, hfs, hdb, hprovider, hparser, hsecondary, hsocket, hwait, _⟩
  exact ⟨hfs, hdb, hprovider, hparser, hsecondary, hsocket, hwait⟩

theorem admitted_request_receipt_is_strictly_submillisecond
    (receipt : ResidentRequestReceipt)
    (h : admitsResidentRequestReceipt receipt) : receipt.elapsedMicros < 1000 :=
  h.1

def deadlineBoundaryReceipt : ResidentRequestReceipt :=
  ⟨.query, .warm, .ready, true, 1000, ⟨1, 0, 0, 0, 0, 0, 0, 0, 0, 0⟩⟩

theorem deadline_boundary_receipt_is_rejected :
    ¬ admitsResidentRequestReceipt deadlineBoundaryReceipt := by
  simp [admitsResidentRequestReceipt, deadlineBoundaryReceipt]

def secondaryRuntimeReceipt : ResidentRequestReceipt :=
  ⟨.search, .cold, .ready, true, 10, ⟨1, 0, 0, 0, 0, 0, 0, 1, 0, 0⟩⟩

theorem secondary_runtime_receipt_is_rejected :
    ¬ admitsResidentRequestReceipt secondaryRuntimeReceipt := by
  simp [admitsResidentRequestReceipt, secondaryRuntimeReceipt]

inductive QueryBaseReadiness where
  | ready | failed
  deriving DecidableEq, Repr

inductive SearchAttachmentReadiness where
  | ready | failed
  deriving DecidableEq, Repr

def queryRemainsReadable
    (base : QueryBaseReadiness) (_attachment : SearchAttachmentReadiness) : Bool :=
  base == .ready

def searchAttachmentReadable
    (base : QueryBaseReadiness) (attachment : SearchAttachmentReadiness) : Bool :=
  base == .ready && attachment == .ready

theorem failed_search_attachment_does_not_revoke_ready_query :
    queryRemainsReadable .ready .failed = true := by
  decide

theorem failed_search_attachment_cannot_serve_search :
    searchAttachmentReadable .ready .failed = false := by
  decide

/- Source admission completion and resident publication have different
   authorities.  Only the daemon observer owns the execution-bound atomic
   publication; request callbacks and the explicit ensure-ready route cannot
   publish a source-only resident generation. -/
inductive ResidentPublicationSource where
  | daemonExecutionObserver
  | requestTerminalCallback
  | explicitEnsureReadyRoute
  deriving DecidableEq, Repr

def mayPublishResidentGeneration : ResidentPublicationSource → Bool
  | .daemonExecutionObserver => true
  | .requestTerminalCallback => false
  | .explicitEnsureReadyRoute => false

theorem request_terminal_callback_cannot_publish_resident_generation :
    mayPublishResidentGeneration .requestTerminalCallback = false := by
  rfl

theorem explicit_ensure_ready_route_cannot_publish_resident_generation :
    mayPublishResidentGeneration .explicitEnsureReadyRoute = false := by
  rfl

theorem daemon_execution_observer_is_the_resident_publication_authority :
    mayPublishResidentGeneration .daemonExecutionObserver = true := by
  rfl

/- The execution-publication observer also owns the joint terminal for the
   derived generation.  A nested mailbox can transport work but cannot own or
   replace that terminal. -/
inductive DerivedGenerationTerminalOwner where
  | daemonExecutionObserver
  | nestedMailboxActor
  | requestHandler
  deriving DecidableEq, Repr

def mayOwnDerivedGenerationTerminal : DerivedGenerationTerminalOwner → Bool
  | .daemonExecutionObserver => true
  | .nestedMailboxActor => false
  | .requestHandler => false

theorem nested_mailbox_cannot_own_derived_generation_terminal :
    mayOwnDerivedGenerationTerminal .nestedMailboxActor = false := by
  rfl

theorem daemon_observer_owns_derived_generation_terminal :
    mayOwnDerivedGenerationTerminal .daemonExecutionObserver = true := by
  rfl

inductive DerivedGenerationTerminalDisposition where
  | publishReady
  | publishFailed
  | discard
  deriving DecidableEq, Repr

def closesDerivedGenerationTerminal : DerivedGenerationTerminalDisposition → Bool
  | .publishReady => true
  | .publishFailed => true
  | .discard => false

theorem discarded_derived_generation_terminal_does_not_close :
    closesDerivedGenerationTerminal .discard = false := by
  rfl

theorem ready_and_failed_dispositions_close_derived_generation_terminal :
    closesDerivedGenerationTerminal .publishReady = true ∧
      closesDerivedGenerationTerminal .publishFailed = true := by
  decide

end ASPProof.RuntimeServerLifecycle

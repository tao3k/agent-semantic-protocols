namespace ASPProof.HookDecisionSingleWriter

inductive PathScope
  | workspace
  | stateHome
  | external
  | unknown
  deriving DecidableEq, Repr

structure RegistrySnapshot where
  generation : Nat
  extensions : String → List String

structure ClassifiedSubject where
  path : String
  languageIds : List String
  scope : PathScope

structure DecisionTransaction where
  transactionId : Nat
  sequence : Nat
  registryGeneration : Nat
  eventCommitted : Bool
  decisionCommitted : Bool

structure RecoveryTopology where
  hookSynchronouslyReconcilesSupervisor : Bool
  recoveryCommandRequiresRuntimeServer : Bool
  recoveryCommandBounded : Bool
  noAgentPassthroughRequiresRuntimeServer : Bool
  residentReentersCliDispatch : Bool
  reconcileWaitsForReady : Bool

structure DrainTopology where
  drainBoundary : Nat
  supervisorBoundary : Nat
  abortsUnresponsiveConnections : Bool
  terminatesOnEvaluatorBoundary : Bool

structure NestedBudgets where
  residentExecution : Nat
  residentBoundary : Nat
  agentBoundary : Nat
  supervisorExecution : Nat
  supervisorBoundary : Nat

structure ControlLaneTopology where
  rejectsClosedGenerationBeforeWrite : Bool
  exchangeBoundary : Nat
  supervisorBoundary : Nat
  discardsFailedLane : Bool
  releasesLaneReservation : Bool

structure RuntimeTaskTopology where
  clientWorkerCount : Nat
  repairOwnsPrivateRuntime : Bool
  repairRunsOnDaemon : Bool

structure CancellableHookTopology where
  immutableSnapshotOnly : Bool
  persistenceOnResponsePath : Bool
  blockingWorkerOnResponsePath : Bool
  taskSurvivesDeadline : Bool
  guardSurvivesDeadline : Bool
  databaseLeaseSurvivesDeadline : Bool

structure LongRunningHookTopology where
  admissionPublishesSnapshot : Bool
  duplicateAdmissionPublications : Nat
  duplicateAdmissionFilesystemProbes : Nat
  identityAdmissionTriggersContentRefresh : Bool
  materializedWorkspacesPerIdentityChange : Nat
  responseMaterializesSnapshot : Bool
  publisherIsSupervised : Bool
  publisherIsJoinedBeforeExit : Bool
  completedRequestRetainsSnapshot : Bool
  descriptorGrowthPerRequest : Nat
  databaseWritesPerRequest : Nat
  restartRequiredForPublication : Bool

structure PolicyCandidate where
  priority : Nat
  decision : Nat
  deriving DecidableEq, Repr

def reservesCleanup (budget : NestedBudgets) : Prop :=
  budget.residentExecution < budget.residentBoundary ∧
  budget.residentBoundary < budget.agentBoundary ∧
  budget.supervisorExecution < budget.supervisorBoundary

def deadlockFreeRecovery (topology : RecoveryTopology) : Prop :=
  topology.hookSynchronouslyReconcilesSupervisor = false ∧
  topology.recoveryCommandRequiresRuntimeServer = false ∧
  topology.recoveryCommandBounded = true ∧
  topology.noAgentPassthroughRequiresRuntimeServer = false ∧
  topology.residentReentersCliDispatch = false ∧
  topology.reconcileWaitsForReady = false

def boundedServerExit (topology : DrainTopology) : Prop :=
  topology.drainBoundary < topology.supervisorBoundary ∧
  topology.abortsUnresponsiveConnections = true ∧
  topology.terminatesOnEvaluatorBoundary = true

def boundedControlLane (topology : ControlLaneTopology) : Prop :=
  topology.rejectsClosedGenerationBeforeWrite = true ∧
  topology.exchangeBoundary < topology.supervisorBoundary ∧
  topology.discardsFailedLane = true ∧
  topology.releasesLaneReservation = true

def daemonOwnedConcurrentTasks (topology : RuntimeTaskTopology) : Prop :=
  1 < topology.clientWorkerCount ∧
  topology.repairOwnsPrivateRuntime = false ∧
  topology.repairRunsOnDaemon = true

def boundedHookEvaluation (topology : CancellableHookTopology) : Prop :=
  topology.immutableSnapshotOnly = true ∧
  topology.persistenceOnResponsePath = false ∧
  topology.blockingWorkerOnResponsePath = false ∧
  topology.taskSurvivesDeadline = false ∧
  topology.guardSurvivesDeadline = false ∧
  topology.databaseLeaseSurvivesDeadline = false

def stableLongRunningHookAuthority (topology : LongRunningHookTopology) : Prop :=
  topology.admissionPublishesSnapshot = true ∧
  topology.duplicateAdmissionPublications = 0 ∧
  topology.duplicateAdmissionFilesystemProbes = 0 ∧
  topology.identityAdmissionTriggersContentRefresh = false ∧
  topology.materializedWorkspacesPerIdentityChange ≤ 1 ∧
  topology.responseMaterializesSnapshot = false ∧
  topology.publisherIsSupervised = true ∧
  topology.publisherIsJoinedBeforeExit = true ∧
  topology.completedRequestRetainsSnapshot = false ∧
  topology.descriptorGrowthPerRequest = 0 ∧
  topology.databaseWritesPerRequest = 0 ∧
  topology.restartRequiredForPublication = false

def selectCandidate (left right : PolicyCandidate) : PolicyCandidate :=
  if left.priority < right.priority then right else left

def classifiesIndependentlyOfScope
    (registry : RegistrySnapshot) (extension : String)
    (left right : ClassifiedSubject) : Prop :=
  left.path.endsWith extension = true →
  right.path.endsWith extension = true →
  left.languageIds = registry.extensions extension ∧
  right.languageIds = registry.extensions extension

def atomicDecision (transaction : DecisionTransaction) : Prop :=
  transaction.eventCommitted = transaction.decisionCommitted

def orderedAfter (left right : DecisionTransaction) : Prop :=
  left.transactionId ≠ right.transactionId → left.sequence ≠ right.sequence

theorem scope_does_not_change_registry_projection
    (registry : RegistrySnapshot) (extension : String)
    (left right : ClassifiedSubject)
    (leftProjection : left.languageIds = registry.extensions extension)
    (rightProjection : right.languageIds = registry.extensions extension) :
    classifiesIndependentlyOfScope registry extension left right := by
  intro _ _
  exact ⟨leftProjection, rightProjection⟩

theorem committed_decision_has_committed_event
    (transaction : DecisionTransaction)
    (atomic : atomicDecision transaction)
    (decision : transaction.decisionCommitted = true) :
    transaction.eventCommitted = true := by
  unfold atomicDecision at atomic
  rw [atomic, decision]

theorem recovery_edge_does_not_wait_on_failed_authority
    (topology : RecoveryTopology)
    (noHotPathReconcile : topology.hookSynchronouslyReconcilesSupervisor = false)
    (bootstrapEscape : topology.recoveryCommandRequiresRuntimeServer = false)
    (boundedControl : topology.recoveryCommandBounded = true)
    (maintenanceEscape : topology.noAgentPassthroughRequiresRuntimeServer = false)
    (directResidentEntry : topology.residentReentersCliDispatch = false)
    (admissionDoesNotWaitReady : topology.reconcileWaitsForReady = false) :
    deadlockFreeRecovery topology := by
  exact ⟨noHotPathReconcile, bootstrapEscape, boundedControl, maintenanceEscape,
    directResidentEntry, admissionDoesNotWaitReady⟩

theorem unresponsive_connection_cannot_own_server_lifetime
    (topology : DrainTopology)
    (withinSupervisor : topology.drainBoundary < topology.supervisorBoundary)
    (abortAtBoundary : topology.abortsUnresponsiveConnections = true)
    (replacePoisonedGeneration : topology.terminatesOnEvaluatorBoundary = true) :
    boundedServerExit topology := by
  exact ⟨withinSupervisor, abortAtBoundary, replacePoisonedGeneration⟩

theorem nested_execution_slices_leave_cleanup_reserve
    (budget : NestedBudgets)
    (residentReserve : budget.residentExecution < budget.residentBoundary)
    (replyReserve : budget.residentBoundary < budget.agentBoundary)
    (supervisorReserve : budget.supervisorExecution < budget.supervisorBoundary) :
    reservesCleanup budget := by
  exact ⟨residentReserve, replyReserve, supervisorReserve⟩

theorem stale_or_stalled_control_lane_cannot_deadlock_supervisor
    (topology : ControlLaneTopology)
    (preflight : topology.rejectsClosedGenerationBeforeWrite = true)
    (nested : topology.exchangeBoundary < topology.supervisorBoundary)
    (discard : topology.discardsFailedLane = true)
    (release : topology.releasesLaneReservation = true) :
    boundedControlLane topology := by
  exact ⟨preflight, nested, discard, release⟩

theorem generation_repair_cannot_collapse_onto_private_single_worker
    (topology : RuntimeTaskTopology)
    (concurrentClient : 1 < topology.clientWorkerCount)
    (noPrivateRuntime : topology.repairOwnsPrivateRuntime = false)
    (daemonOwnership : topology.repairRunsOnDaemon = true) :
    daemonOwnedConcurrentTasks topology := by
  exact ⟨concurrentClient, noPrivateRuntime, daemonOwnership⟩

theorem caller_timeout_leaves_no_io_or_writer_authority
    (topology : CancellableHookTopology)
    (snapshotOnly : topology.immutableSnapshotOnly = true)
    (noPersistence : topology.persistenceOnResponsePath = false)
    (noBlockingWorker : topology.blockingWorkerOnResponsePath = false)
    (noTask : topology.taskSurvivesDeadline = false)
    (noGuard : topology.guardSurvivesDeadline = false)
    (noDatabaseLease : topology.databaseLeaseSurvivesDeadline = false) :
    boundedHookEvaluation topology := by
  exact ⟨snapshotOnly, noPersistence, noBlockingWorker, noTask, noGuard,
    noDatabaseLease⟩

theorem completed_requests_cannot_accumulate_runtime_authority
    (topology : LongRunningHookTopology)
    (controlPlanePublication : topology.admissionPublishesSnapshot = true)
    (duplicateAdmissionIsSilent : topology.duplicateAdmissionPublications = 0)
    (duplicateAdmissionHasNoIo : topology.duplicateAdmissionFilesystemProbes = 0)
    (authoritySeparation : topology.identityAdmissionTriggersContentRefresh = false)
    (targetedMaterialization : topology.materializedWorkspacesPerIdentityChange ≤ 1)
    (noResponseMaterialization : topology.responseMaterializesSnapshot = false)
    (supervised : topology.publisherIsSupervised = true)
    (joined : topology.publisherIsJoinedBeforeExit = true)
    (noRetainedSnapshot : topology.completedRequestRetainsSnapshot = false)
    (noDescriptorGrowth : topology.descriptorGrowthPerRequest = 0)
    (noDatabaseWrites : topology.databaseWritesPerRequest = 0)
    (noRestartConsistency : topology.restartRequiredForPublication = false) :
    stableLongRunningHookAuthority topology := by
  exact ⟨controlPlanePublication, duplicateAdmissionIsSilent, duplicateAdmissionHasNoIo,
    authoritySeparation, targetedMaterialization, noResponseMaterialization, supervised,
    joined, noRetainedSnapshot, noDescriptorGrowth, noDatabaseWrites,
    noRestartConsistency⟩

theorem higher_priority_candidate_wins
    (left right : PolicyCandidate)
    (higher : left.priority < right.priority) :
    selectCandidate left right = right := by
  simp [selectCandidate, higher]

end ASPProof.HookDecisionSingleWriter

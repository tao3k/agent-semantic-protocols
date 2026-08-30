namespace ASPProof.CodexMultiAgentV2NamespaceResumption

inductive DurableNamespaceState where
  | absent
  | present
  deriving DecidableEq, Repr

inductive HostChildLiveness where
  | running
  | idle
  | terminated
  deriving DecidableEq, Repr

inductive CollaborationDecision where
  | createNew
  | continueExisting
  deriving DecidableEq, Repr

inductive NamespaceAction where
  | create
  | resume
  deriving DecidableEq, Repr

inductive CodexHostAction where
  | spawnAgent
  | followupTask
  deriving DecidableEq, Repr

structure NamespaceKey where
  projectId : String
  workspaceId : String
  canonicalWorkspaceRoot : String
  platform : String
  platformSessionId : String
  rootNamespaceId : String
  registeredAgentName : String
  deriving DecidableEq, Repr

def mayBindNamespace (requested durable : NamespaceKey) : Bool :=
  requested == durable

def chooseAction
    (namespaceState : DurableNamespaceState)
    (_hostLiveness : HostChildLiveness) :
      NamespaceAction × CollaborationDecision × CodexHostAction :=
  match namespaceState with
  | .absent => (.create, .createNew, .spawnAgent)
  | .present => (.resume, .continueExisting, .followupTask)

theorem create_iff_namespace_absent
    (namespaceState : DurableNamespaceState)
    (hostLiveness : HostChildLiveness) :
    chooseAction namespaceState hostLiveness =
      (.create, .createNew, .spawnAgent) ↔
      namespaceState = .absent := by
  cases namespaceState <;> simp [chooseAction]

theorem existing_namespace_always_resumes
    (hostLiveness : HostChildLiveness) :
    chooseAction .present hostLiveness =
      (.resume, .continueExisting, .followupTask) := by
  cases hostLiveness <;> rfl

theorem terminated_child_does_not_imply_namespace_absence :
    chooseAction .present .terminated =
      (.resume, .continueExisting, .followupTask) := by
  rfl

theorem choice_is_independent_of_host_liveness
    (namespaceState : DurableNamespaceState)
    (left right : HostChildLiveness) :
    chooseAction namespaceState left = chooseAction namespaceState right := by
  cases namespaceState <;> rfl

theorem a_different_root_session_cannot_bind
    (projectId workspaceId canonicalWorkspaceRoot platform requestedSession durableSession
      requestedRoot durableRoot registeredAgentName : String)
    (differentSession : requestedSession ≠ durableSession) :
    mayBindNamespace
      ⟨projectId, workspaceId, canonicalWorkspaceRoot, platform, requestedSession,
        requestedRoot, registeredAgentName⟩
      ⟨projectId, workspaceId, canonicalWorkspaceRoot, platform, durableSession,
        durableRoot, registeredAgentName⟩ = false := by
  simp [mayBindNamespace, differentSession]

theorem a_different_workspace_cannot_bind
    (projectId requestedWorkspace durableWorkspace canonicalWorkspaceRoot platform
      platformSessionId rootNamespaceId registeredAgentName : String)
    (differentWorkspace : requestedWorkspace ≠ durableWorkspace) :
    mayBindNamespace
      ⟨projectId, requestedWorkspace, canonicalWorkspaceRoot, platform, platformSessionId,
        rootNamespaceId, registeredAgentName⟩
      ⟨projectId, durableWorkspace, canonicalWorkspaceRoot, platform, platformSessionId,
        rootNamespaceId, registeredAgentName⟩ = false := by
  simp [mayBindNamespace, differentWorkspace]

theorem one_workspace_identity_cannot_bind_two_canonical_roots
    (projectId workspaceId requestedRoot durableRoot platform platformSessionId
      rootNamespaceId registeredAgentName : String)
    (differentRoot : requestedRoot ≠ durableRoot) :
    mayBindNamespace
      ⟨projectId, workspaceId, requestedRoot, platform, platformSessionId,
        rootNamespaceId, registeredAgentName⟩
      ⟨projectId, workspaceId, durableRoot, platform, platformSessionId,
        rootNamespaceId, registeredAgentName⟩ = false := by
  simp [mayBindNamespace, differentRoot]

end ASPProof.CodexMultiAgentV2NamespaceResumption

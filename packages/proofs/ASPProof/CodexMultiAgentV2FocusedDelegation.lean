-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.CodexMultiAgentV2RegistryMaterialization

namespace ASPProof.CodexMultiAgentV2FocusedDelegation

open ASPProof.CodexMultiAgentV2RegistryMaterialization

inductive DelegationCapability where
  | standard
  | focusedLeaf
deriving DecidableEq

structure ControlAgent where
  registry : RegistryAgent
  capability : DelegationCapability
deriving DecidableEq

structure DelegationEdge where
  parentSessionId : String
  childSessionId : String
deriving DecidableEq

structure DelegationState where
  agents : List ControlAgent
  delegations : List DelegationEdge
deriving DecidableEq

structure DelegationProposal where
  parentSessionId : String
  child : ControlAgent
deriving DecidableEq

inductive DelegationDenialReason where
  | parentMissing
  | focusedLeaf
deriving DecidableEq

inductive DelegationDecision where
  | denied (reason : DelegationDenialReason)
  | accepted
deriving DecidableEq

structure DelegationAdmissionReceipt where
  decision : DelegationDecision
  nextState : DelegationState
deriving DecidableEq

def currentAgent
    (state : DelegationState)
    (sessionId : String) : Option ControlAgent :=
  state.agents.find? (fun agent => agent.registry.sessionId == sessionId)

def admitDelegation
    (state : DelegationState)
    (proposal : DelegationProposal) : DelegationAdmissionReceipt :=
  match currentAgent state proposal.parentSessionId with
  | none =>
      { decision := .denied .parentMissing, nextState := state }
  | some current =>
      match current.capability with
      | .focusedLeaf =>
          { decision := .denied .focusedLeaf, nextState := state }
      | .standard =>
          { decision := .accepted
            nextState :=
              { agents := state.agents ++ [proposal.child]
                delegations := state.delegations ++
                  [{ parentSessionId := current.registry.sessionId
                     childSessionId := proposal.child.registry.sessionId }] } }

theorem missingParentRejectsAndPreservesState
    (state : DelegationState)
    (proposal : DelegationProposal)
    (missing : currentAgent state proposal.parentSessionId = none) :
    admitDelegation state proposal =
      { decision := .denied .parentMissing, nextState := state } := by
  simp [admitDelegation, missing]

theorem focusedLeafAlwaysRejects
    (state : DelegationState)
    (proposal : DelegationProposal)
    (current : ControlAgent)
    (found : currentAgent state proposal.parentSessionId = some current)
    (leaf : current.capability = .focusedLeaf) :
    admitDelegation state proposal =
      { decision := .denied .focusedLeaf, nextState := state } := by
  simp [admitDelegation, found, leaf]

theorem focusedLeafRejectPreservesAgents
    (state : DelegationState)
    (proposal : DelegationProposal)
    (current : ControlAgent)
    (found : currentAgent state proposal.parentSessionId = some current)
    (leaf : current.capability = .focusedLeaf) :
    (admitDelegation state proposal).nextState.agents = state.agents := by
  simp [admitDelegation, found, leaf]

theorem focusedLeafRejectPreservesDelegations
    (state : DelegationState)
    (proposal : DelegationProposal)
    (current : ControlAgent)
    (found : currentAgent state proposal.parentSessionId = some current)
    (leaf : current.capability = .focusedLeaf) :
    (admitDelegation state proposal).nextState.delegations = state.delegations := by
  simp [admitDelegation, found, leaf]

theorem standardAgentAcceptsOneChildEdge
    (state : DelegationState)
    (proposal : DelegationProposal)
    (current : ControlAgent)
    (found : currentAgent state proposal.parentSessionId = some current)
    (standard : current.capability = .standard) :
    admitDelegation state proposal =
      { decision := .accepted
        nextState :=
          { agents := state.agents ++ [proposal.child]
            delegations := state.delegations ++
              [{ parentSessionId := current.registry.sessionId
                 childSessionId := proposal.child.registry.sessionId }] } } := by
  simp [admitDelegation, found, standard]

end ASPProof.CodexMultiAgentV2FocusedDelegation

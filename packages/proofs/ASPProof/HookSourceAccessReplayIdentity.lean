-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.HookSourceAccessReplayIdentity

inductive HostEnvelope
  | structuredCode (code : String)
  | freeformCode (code : String)
  deriving DecidableEq

def projectedCode : HostEnvelope → String
  | .structuredCode code => code
  | .freeformCode code => code

theorem freeformEnvelopePreservesNestedAction (code : String) :
    projectedCode (.freeformCode code) = code := by
  rfl

structure SourceAccessLane where
  reasonKind : String
  languageId : String
  ownerPath : String
  routeCommand : String
  deriving DecidableEq

def sameReplayLane (left right : SourceAccessLane) : Bool :=
  decide (left = right)

theorem distinctOwnerCannotCollapseReplayLane
    (left right : SourceAccessLane)
    (differentOwner : left.ownerPath ≠ right.ownerPath) :
    sameReplayLane left right = false := by
  simp [sameReplayLane]
  intro equalLane
  exact differentOwner (congrArg SourceAccessLane.ownerPath equalLane)

theorem distinctLanguageCannotCollapseReplayLane
    (left right : SourceAccessLane)
    (differentLanguage : left.languageId ≠ right.languageId) :
    sameReplayLane left right = false := by
  simp [sameReplayLane]
  intro equalLane
  exact differentLanguage (congrArg SourceAccessLane.languageId equalLane)

structure DenyGuidance where
  naturalLanguage : String
  routeCommand : String
  deriving DecidableEq

def replayGuidance (guidance : DenyGuidance) : DenyGuidance := guidance

theorem replayPreservesExecutableRoute (guidance : DenyGuidance) :
    (replayGuidance guidance).routeCommand = guidance.routeCommand := by
  rfl

theorem replayPreservesNaturalLanguage (guidance : DenyGuidance) :
    (replayGuidance guidance).naturalLanguage = guidance.naturalLanguage := by
  rfl

end ASPProof.HookSourceAccessReplayIdentity

-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.AgentSessionBootstrapProgress

namespace ASPProof.Audit.AgentSessionBootstrapProgress

open ASPProof.AgentSessionBootstrapProgress

def typedIpcReady : Observation :=
  { exitCode := 0
    terminal := .ready
    registryEntry := true
    routable := true
    commandRequested := true
    commandReceipt := true
    requestIdentityMatches := true
    directOpenAttempted := false }

def directOpenReadyClaim : Observation :=
  { typedIpcReady with directOpenAttempted := true }

example : successfulExit typedIpcReady := by
  simp [successfulExit, readyEvidence, typedIpcReady]

example : ¬ readyEvidence directOpenReadyClaim := by
  simp [readyEvidence, directOpenReadyClaim, typedIpcReady]

example : ¬ successfulExit emptySuccessCounterexample :=
  emptySuccessIsRejected

end ASPProof.Audit.AgentSessionBootstrapProgress

-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.AgentSessionBootstrapDispatchPriority

inductive RegistryBinding where | exact | absent | stale deriving DecidableEq
inductive HostPath where | present | absent | unobserved deriving DecidableEq
inductive HostTask where | absent | idle | running | completed | errored | unobserved deriving DecidableEq
inductive BootstrapState where | notStarted | pending | registered | typedFailure | emptyPayload deriving DecidableEq
inductive DispatchAction where | followup | registerExisting | spawn | observeHostPath | awaitTerminal | repairExisting deriving DecidableEq

def choose : RegistryBinding → HostPath → HostTask → BootstrapState → DispatchAction
  | _, _, .completed, .emptyPayload => .repairExisting
  | _, _, .errored, .emptyPayload => .repairExisting
  | _, _, _, .pending => .awaitTerminal
  | _, _, _, .emptyPayload => .awaitTerminal
  | _, _, _, .typedFailure => .repairExisting
  | .exact, .present, _, .registered => .followup
  | .absent, .absent, _, .notStarted => .spawn
  | .stale, .absent, _, .notStarted => .repairExisting
  | .exact, .absent, _, .notStarted => .repairExisting
  | _, .present, _, .notStarted => .registerExisting
  | _, .unobserved, _, .notStarted => .observeHostPath
  | _, _, _, .registered => .repairExisting

theorem exactBindingPrioritizesFollowup (task : HostTask) :
    choose .exact .present task .registered = .followup := by
  cases task <;> rfl

theorem emptyPayloadNeverAuthorizesSpawn
    (binding : RegistryBinding) (path : HostPath) (task : HostTask) :
    choose binding path task .emptyPayload ≠ .spawn := by
  cases binding <;> cases path <;> cases task <;> decide

theorem emptyPayloadNeverAuthorizesUnboundFollowup
    (path : HostPath) (task : HostTask) :
    choose .absent path task .emptyPayload ≠ .followup := by
  cases path <;> cases task <;> decide

theorem activePendingBootstrapOnlyWaits
    (binding : RegistryBinding) (path : HostPath) :
    choose binding path .running .pending = .awaitTerminal := by
  cases binding <;> cases path <;> rfl

theorem completedEmptyPayloadRepairsSameChild
    (binding : RegistryBinding) (path : HostPath) :
    choose binding path .completed .emptyPayload = .repairExisting := by rfl

theorem presentUnregisteredPathIsNeverRespawned :
    choose .absent .present .idle .notStarted = .registerExisting := by rfl

end ASPProof.AgentSessionBootstrapDispatchPriority

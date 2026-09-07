-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.ExactProjectionTerminalAuthority

inductive ProviderKind where
  | programmingLanguage
  | document
  deriving DecidableEq

inductive GenerationAuthority where
  | admitted
  | unavailable
  | recoveryRequired
  deriving DecidableEq

inductive ExactRoute where
  | residentEnvelope
  | terminalFailure
  | publicProviderArgv
  deriving DecidableEq

structure RegisteredProvider where
  languageId : String
  kind : ProviderKind

def routeExact
    (provider : RegisteredProvider)
    (authority : GenerationAuthority) : ExactRoute :=
  match provider.kind, authority with
  | .programmingLanguage, .admitted => .residentEnvelope
  | .programmingLanguage, .unavailable => .terminalFailure
  | .programmingLanguage, .recoveryRequired => .terminalFailure
  | .document, _ => .terminalFailure

theorem programmingLanguageExactNeverUsesPublicProviderArgv
    (provider : RegisteredProvider)
    (authority : GenerationAuthority)
    (programming : provider.kind = .programmingLanguage) :
    routeExact provider authority ≠ .publicProviderArgv := by
  cases authority <;> simp [routeExact, programming]

theorem unavailableGenerationIsTerminal
    (provider : RegisteredProvider)
    (programming : provider.kind = .programmingLanguage) :
    routeExact provider .unavailable = .terminalFailure := by
  simp [routeExact, programming]

theorem recoveryRequiredGenerationIsTerminal
    (provider : RegisteredProvider)
    (programming : provider.kind = .programmingLanguage) :
    routeExact provider .recoveryRequired = .terminalFailure := by
  simp [routeExact, programming]

end ASPProof.ExactProjectionTerminalAuthority

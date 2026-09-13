-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.HookToolMatcherCoverage

inductive HostToolAction where
  | bash
  | read
  | applyPatch
  | mcp
  | other
  deriving DecidableEq, Repr

inductive RegisteredDocumentExtension where
  | org
  | md
  deriving DecidableEq, Repr

/-- Codex match-all delivery is the host boundary before ASP language classification. -/
def wildcardMatcher (_ : HostToolAction) : Bool := true

def bashOnlyMatcher (action : HostToolAction) : Bool :=
  action == .bash

def reachesLanguageExtensionMatcher
    (hostMatcher : HostToolAction → Bool)
    (action : HostToolAction)
    (_extension : RegisteredDocumentExtension) : Bool :=
  hostMatcher action

theorem wildcard_delivers_every_supported_tool (action : HostToolAction) :
    wildcardMatcher action = true := by
  rfl

theorem wildcard_delivers_org_read :
    reachesLanguageExtensionMatcher wildcardMatcher .read .org = true := by
  rfl

theorem wildcard_delivers_md_read :
    reachesLanguageExtensionMatcher wildcardMatcher .read .md = true := by
  rfl

theorem nonempty_bash_matcher_does_not_deliver_org_read :
    reachesLanguageExtensionMatcher bashOnlyMatcher .read .org = false := by
  decide

theorem nonempty_bash_matcher_does_not_deliver_md_read :
    reachesLanguageExtensionMatcher bashOnlyMatcher .read .md = false := by
  decide

end ASPProof.HookToolMatcherCoverage

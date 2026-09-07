-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.HostAuthoritativeAgentProfile

open ASPProof.HostAuthoritativeAgentProfile

example
    (receipt : HostReceipt) (left right : LegacyManagerProjection) :
    admitWithLegacyIgnored receipt left = admitWithLegacyIgnored receipt right :=
  admission_independent_of_legacy_projection receipt left right

example :
    let receipt : HostReceipt := {
      typedRoleMatches := true
      uniqueCanonicalPath := true
      liveBindingFresh := true
    }
    let stale : LegacyManagerProjection := {
      expectedModel := "legacy-model"
      expectedReasoning := "low"
    }
    admit receipt = .ready ∧
      legacyAdmit receipt stale "host-model" "low" = .residentCommandBlocked :=
  legacy_shadow_projection_can_block_valid_host_receipt

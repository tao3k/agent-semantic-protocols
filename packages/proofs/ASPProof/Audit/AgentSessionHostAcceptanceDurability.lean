-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.AgentSessionHostAcceptanceDurability

namespace ASPProof.Audit.AgentSessionHostAcceptanceDurability

open ASPProof.AgentSessionHostAcceptanceDurability

#print axioms durable_first_accept_constructible
#print axioms durable_retry_preserves_effect
#print axioms crash_restart_preserves_registry
#print axioms accepted_registry_cannot_restart_empty
#print axioms volatile_reset_allows_duplicate_effect_trace
#print axioms accepted_rotation_preserves_effect
#print axioms accepted_rotation_preserves_host_sequence
#print axioms accepted_rotation_cannot_become_empty
#print axioms missing_continuity_quarantines
#print axioms missing_continuity_cannot_resume
#print axioms accepted_registry_contains_receipt
#print axioms empty_registry_cannot_verify_receipt
#print axioms verified_receipt_can_finalize
#print axioms empty_registry_cannot_finalize

end ASPProof.Audit.AgentSessionHostAcceptanceDurability

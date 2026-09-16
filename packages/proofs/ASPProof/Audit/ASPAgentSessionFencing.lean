-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.ASPAgentSessionFencing

open ASPProof.ASPAgentSessionFencing

#print axioms staleSessionEpochRejectsCompletion
#print axioms staleAgentGenerationRejectsCompletion
#print axioms staleTurnTokenRejectsCompletion
#print axioms currentFencedCompletionIsAdmissible
#print axioms drainingRejectsExternalCommand
#print axioms staleObservationCannotBecomeAuthority
#print axioms currentObservationMatchesAuthority
#print axioms enqueueMessageOnceIsIdempotent
#print axioms consumeReceiptOnceIsIdempotent

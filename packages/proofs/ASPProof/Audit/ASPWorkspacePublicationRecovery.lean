-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.ASPWorkspacePublicationRecovery

open ASPProof.ASPWorkspacePublicationRecovery

#print axioms advanceDoesNotRegress
#print axioms advancePreservesStableKey
#print axioms timeoutPreservesPublicationPhase
#print axioms advanceFivePreservesStableKey
#print axioms fiveSuccessfulAdvancesDeliver
#print axioms duplicateReceiptIndexKeepsFirst
#print axioms deliverPreservesStableKey
#print axioms oldDigestReceiptRejectedAfterRetarget

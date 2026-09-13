-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.RuntimeReadQueryAuthority

open ASPProof.RuntimeReadQueryAuthority

#print axioms readOnlyResidentConnectsGlobalRuntime
#print axioms readOnlyResidentCannotConnectWorkspaceOwner
#print axioms internalRouteNeverExposesOwnerEndpoint
#print axioms readQueryDoesNotPublishGeneration
#print axioms readQueryDoesNotMutateOverlay
#print axioms changedOwnerRejectsCachedProjection
#print axioms providerNativeFallbackRequiresDeclaration
#print axioms finiteBudgetQueryHasOnlyTerminalOutcomes
#print axioms unavailableOutcomeIsActionable

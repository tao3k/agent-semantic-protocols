-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.ASPWorkspaceGenerationReadiness

open ASPProof.ASPWorkspaceGenerationReadiness

#print axioms serverHealthPreservesMissingGeneration
#print axioms registrationDoesNotCreateGeneration
#print axioms parentPublicationLeavesNestedWorkspaceUnchanged
#print axioms missingGenerationRejectsExactProjection
#print axioms digestMismatchRejectsExactProjection
#print axioms missingProjectionRejectsExactQuery
#print axioms canonicalPublicationAdmitsEveryProjection
#print axioms queuedSnapshotIsNotReadBarrier
#print axioms targetedPublicationCannotAdmitRead
#print axioms replayedReadyTerminalDoesNotRequireEnqueueAccepted
#print axioms workspaceRepairPreservesAgentAuthority

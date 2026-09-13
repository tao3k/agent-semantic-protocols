-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.RuntimeArtifactAuthority

open ASPProof.RuntimeArtifactAuthority

example : originAdmitted .dev .lockedRelease = false :=
  dev_never_admits_locked_release

example (receipt : ArtifactReceipt) (h : receipt.origin = .lockedRelease) :
    resolveDev (some receipt) = .devArtifactUnavailable :=
  release_candidate_fails_closed_in_dev_model receipt h

example : residentWarmPath ⟨0, 0, 0, 0, 0⟩ :=
  canonical_warm_receipt_is_resident

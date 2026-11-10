-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchResultOwnerSupportIndex

open ASPProof.SearchResultOwnerSupportIndex

#check graph_cannot_admit_unsupported_owner
#check graph_preserves_acquisition_supported_owner
#check indexed_support_work_does_not_exceed_nested_scan

#print axioms graph_cannot_admit_unsupported_owner
#print axioms graph_preserves_acquisition_supported_owner
#print axioms indexed_support_work_does_not_exceed_nested_scan

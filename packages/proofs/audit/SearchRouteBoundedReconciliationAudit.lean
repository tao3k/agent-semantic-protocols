-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.SearchRouteBoundedReconciliation

open ASPProof.Audit.SearchRouteBoundedReconciliation

def main : IO Unit :=
  IO.println auditManifest.compress

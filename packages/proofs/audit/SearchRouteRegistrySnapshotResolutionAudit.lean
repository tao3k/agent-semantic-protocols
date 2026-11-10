-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.SearchRouteRegistrySnapshotResolution

def main : IO Unit :=
  IO.println ASPProof.Audit.SearchRouteRegistrySnapshotResolution.receipt.compress

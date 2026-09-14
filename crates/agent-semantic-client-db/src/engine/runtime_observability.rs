// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::{path::Path, sync::Arc};

/// Returns the DB Engine-owned shared database used by the Runtime Server's
/// telemetry exporter. This purpose-limited API keeps pooling and database
/// construction inside the storage engine without exposing its generic pool.
pub async fn runtime_observability_database(path: &Path) -> Result<Arc<::turso::Database>, String> {
    super::turso::shared_turso_database(path).await
}

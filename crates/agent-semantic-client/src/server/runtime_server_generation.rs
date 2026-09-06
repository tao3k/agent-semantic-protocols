// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Runtime Server generation mutation control-plane adapters.

use std::path::Path;

pub(crate) async fn submit_runtime_generation_mutation(
    project_root: &Path,
    mutation_id: String,
    changed_paths: Vec<String>,
) -> Result<serde_json::Value, String> {
    let project_root = project_root.to_path_buf();
    let session =
        super::runtime_server::runtime_server_workspace_session_for_admission_async(&project_root)
            .await?;
    let receipt = session
        .submit_runtime_generation_mutation(mutation_id, changed_paths)
        .await?;
    receipt.validate()?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("failed to encode runtime generation admission: {error}"))
}

// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::fs;
use std::path::PathBuf;

use agent_semantic_schema_manager::SchemaManager;
use agent_semantic_schema_manager::build_support::provider_registry::resolve_provider_register;

#[tokio::main]
async fn main() -> Result<(), String> {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let resolved = resolve_provider_register(&workspace)?;
    let resolved_path =
        workspace.join("crates/agent-semantic-provider-protocol/provider-register.resolved.json");
    let mut bytes = resolved.bytes;
    bytes.push(b'\n');
    fs::write(&resolved_path, bytes)
        .map_err(|error| format!("write {}: {error}", resolved_path.display()))?;
    println!("updated {}", resolved_path.display());
    let manager = SchemaManager::new(&workspace);
    let reports = manager.materialize(&[]).await?;
    for report in &reports {
        println!(
            "materialized {}: schemas={}, changed={}, removed={}, digest={}",
            report.language_id,
            report.schema_count,
            report.changed_count,
            report.removed_count,
            report.bundle_digest
        );
    }
    let verified = manager.verify(&[]).await?;
    println!("verified {} language bundles cleanly", verified.len());
    Ok(())
}

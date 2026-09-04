//! Runtime schema catalog compilation for thin downstream build scripts.

use std::collections::BTreeMap;
use std::path::Path;

use crate::SchemaManager;

/// Compile the immutable Runtime schema catalog from canonical schema-manager facts.
pub async fn compile_runtime_schema_catalog(
    workspace_root: &Path,
    output: &Path,
) -> Result<(), String> {
    let manager = SchemaManager::new(workspace_root);
    let profiles = manager.registered_language_profiles()?;
    for profile in &profiles {
        println!(
            "cargo:rerun-if-changed={}",
            workspace_root.join(&profile.bundle_root).display()
        );
    }
    let responsibilities = manager
        .responsibilities()
        .await?
        .into_iter()
        .map(|responsibility| (responsibility.name, responsibility.family_id))
        .collect::<BTreeMap<_, _>>();
    let resolved = manager.resolve_bundles(&[]).await?;
    let mut bundles = Vec::with_capacity(resolved.len());
    for bundle in resolved {
        let mut entries = Vec::with_capacity(bundle.schemas.len());
        for owned in bundle.schemas {
            let bytes = owned.bytes;
            let document: serde_json::Value = serde_json::from_slice(&bytes).map_err(|error| {
                format!("decode verified schema document {}: {error}", owned.name)
            })?;
            let schema_id = document
                .get("$id")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| format!("schema document has no $id: {}", owned.name))?;
            let family_id = responsibilities.get(&owned.name).ok_or_else(|| {
                format!(
                    "schema document has no SchemaManager family: {}",
                    owned.name
                )
            })?;
            entries.push(serde_json::json!({
                "familyId": family_id,
                "schemaId": schema_id,
                "schemaVersion": "1",
                "name": owned.name,
                "digest": owned.digest,
                "document": document,
            }));
        }
        bundles.push(serde_json::json!({
            "languageId": bundle.language_id,
            "rootSetIds": bundle.root_set_ids,
            "bundleDigest": bundle.bundle_digest,
            "entries": entries,
        }));
    }
    bundles.sort_by(|left, right| {
        left["languageId"]
            .as_str()
            .cmp(&right["languageId"].as_str())
    });
    let bytes = serde_json::to_vec(&serde_json::json!({
        "schemaId": "agent.semantic-protocols.runtime-schema-bundle-catalog",
        "schemaVersion": 1,
        "bundles": bundles,
    }))
    .map_err(|error| format!("encode Runtime schema catalog: {error}"))?;
    std::fs::write(output, bytes)
        .map_err(|error| format!("write Runtime schema catalog {}: {error}", output.display()))
}

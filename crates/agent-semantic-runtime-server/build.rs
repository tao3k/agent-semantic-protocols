use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=proto/asp-provider-stream.proto");
    println!("cargo:rerun-if-changed=proto/asp-python-graphs.proto");
    println!("cargo:rerun-if-changed=../../schemas");

    let manifest_dir = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("runtime-server manifest directory"),
    );
    let workspace_root = manifest_dir.join("../..");
    let output = PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo OUT_DIR"))
        .join("runtime-schema-bundles.v1.json");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("schema catalog build runtime");
    runtime
        .block_on(compile_runtime_schema_catalog(&workspace_root, &output))
        .expect("compile immutable Runtime schema catalog");

    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(
            &[
                "proto/asp-provider-stream.proto",
                "proto/asp-python-graphs.proto",
            ],
            &["proto"],
        )
        .expect("compile runtime-server gRPC protocols");
}

async fn compile_runtime_schema_catalog(
    workspace_root: &Path,
    output: &Path,
) -> Result<(), String> {
    let manager = agent_semantic_schema_manager::SchemaManager::new(workspace_root);
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

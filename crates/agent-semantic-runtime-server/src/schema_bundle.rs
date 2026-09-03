//! Immutable Runtime projection of SchemaManager-owned language bundles.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent_semantic_client_protocol::{
    SCHEMA_BUNDLE_RESPONSE_SCHEMA_ID, SCHEMA_VERSION, SchemaBundleDocument, SchemaBundleEntry,
    SchemaBundleReceipt, SchemaBundleRequest, SchemaBundleResponse,
};
use agent_semantic_schema_manager::{
    BUNDLE_RECEIPT_FILE, SchemaManager, load_verified_bundle_receipt,
};
use serde::Deserialize;

const EMBEDDED_SCHEMA_CATALOG: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/runtime-schema-bundles.v1.json"));

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EmbeddedSchemaCatalog {
    schema_id: String,
    schema_version: u64,
    bundles: Vec<EmbeddedLanguageBundle>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EmbeddedLanguageBundle {
    language_id: String,
    root_set_ids: Vec<String>,
    bundle_digest: String,
    entries: Vec<EmbeddedSchemaEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EmbeddedSchemaEntry {
    family_id: String,
    schema_id: String,
    schema_version: String,
    name: String,
    digest: String,
    document: serde_json::Value,
}

#[derive(Clone)]
struct VerifiedLanguageBundle {
    root_set_ids: Arc<[String]>,
    bundle_digest: String,
    ready: Arc<SchemaBundleResponse>,
    unchanged: Arc<SchemaBundleResponse>,
    profile_mismatch: Arc<SchemaBundleResponse>,
}

/// Startup-built, immutable projection used by the public ClientFrame route.
#[derive(Clone, Default)]
pub struct RuntimeSchemaBundleCatalog {
    bundles: Arc<HashMap<String, VerifiedLanguageBundle>>,
}

impl RuntimeSchemaBundleCatalog {
    /// Decode the build-verified catalog embedded in the Runtime artifact.
    /// Runtime startup never scans, materializes, or verifies the mutable
    /// source checkout; changing any schema changes the enclosing binary
    /// digest and therefore requires a new active bundle publication.
    pub fn load_embedded() -> Result<Self, String> {
        let embedded: EmbeddedSchemaCatalog = serde_json::from_slice(EMBEDDED_SCHEMA_CATALOG)
            .map_err(|error| format!("decode embedded Runtime schema catalog: {error}"))?;
        if embedded.schema_id != "agent.semantic-protocols.runtime-schema-bundle-catalog"
            || embedded.schema_version != 1
            || embedded.bundles.is_empty()
        {
            return Err("invalid embedded Runtime schema catalog authority".to_owned());
        }
        let mut bundles = HashMap::with_capacity(embedded.bundles.len());
        for bundle in embedded.bundles {
            let mut entries = Vec::with_capacity(bundle.entries.len());
            let mut documents = Vec::with_capacity(bundle.entries.len());
            for embedded_entry in bundle.entries {
                let entry = SchemaBundleEntry {
                    family_id: embedded_entry.family_id,
                    schema_id: embedded_entry.schema_id,
                    schema_version: embedded_entry.schema_version,
                    name: embedded_entry.name,
                    digest: embedded_entry.digest,
                };
                documents.push(SchemaBundleDocument {
                    entry: entry.clone(),
                    document: embedded_entry.document,
                });
                entries.push(entry);
            }
            let receipt = SchemaBundleReceipt {
                language_id: bundle.language_id.clone(),
                root_set_ids: bundle.root_set_ids.clone(),
                bundle_digest: bundle.bundle_digest.clone(),
            };
            let ready = Arc::new(SchemaBundleResponse::Ready {
                schema_id: SCHEMA_BUNDLE_RESPONSE_SCHEMA_ID.to_owned(),
                schema_version: SCHEMA_VERSION.to_owned(),
                receipt: receipt.clone(),
                entries: entries.clone(),
                documents,
            });
            let unchanged = Arc::new(SchemaBundleResponse::Unchanged {
                schema_id: SCHEMA_BUNDLE_RESPONSE_SCHEMA_ID.to_owned(),
                schema_version: SCHEMA_VERSION.to_owned(),
                receipt: receipt.clone(),
                entries,
            });
            let profile_mismatch = Arc::new(failed(
                &bundle.language_id,
                "schema-bundle-profile-mismatch",
                serde_json::json!({"action": "use-registered-root-set-profile"}),
                serde_json::json!({"registeredRootSetIds": bundle.root_set_ids}),
            ));
            ready.validate()?;
            unchanged.validate()?;
            profile_mismatch.validate()?;
            if bundles
                .insert(
                    bundle.language_id.clone(),
                    VerifiedLanguageBundle {
                        root_set_ids: Arc::from(receipt.root_set_ids),
                        bundle_digest: receipt.bundle_digest,
                        ready,
                        unchanged,
                        profile_mismatch,
                    },
                )
                .is_some()
            {
                return Err(format!(
                    "duplicate embedded Runtime schema bundle: {}",
                    bundle.language_id
                ));
            }
        }
        Ok(Self {
            bundles: Arc::new(bundles),
        })
    }

    /// Bind exactly the language schema bundles required by one workspace.
    pub fn binding_digest<'a>(
        &self,
        language_ids: impl IntoIterator<Item = &'a str>,
    ) -> Result<String, String> {
        let mut language_ids = language_ids.into_iter().collect::<Vec<_>>();
        language_ids.sort_unstable();
        language_ids.dedup();
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"agent.semantic-protocols.runtime-schema-bundle-binding.v1\0");
        for language_id in language_ids {
            let bundle = self.bundles.get(language_id).ok_or_else(|| {
                format!("Runtime schema bundle is absent for required language `{language_id}`")
            })?;
            hasher.update(language_id.as_bytes());
            hasher.update(b"\0");
            hasher.update(bundle.bundle_digest.as_bytes());
            hasher.update(b"\0");
        }
        Ok(format!("blake3-256:{}", hasher.finalize().to_hex()))
    }

    pub async fn load(workspace_root: impl Into<PathBuf>) -> Result<Self, String> {
        let workspace_root = workspace_root.into();
        let manager = SchemaManager::new(&workspace_root);
        let profiles = manager.registered_language_profiles()?;
        let responsibilities = manager
            .responsibilities()
            .await?
            .into_iter()
            .map(|responsibility| (responsibility.name, responsibility.family_id))
            .collect::<BTreeMap<_, _>>();
        manager.materialize(&[]).await?;
        let reports = manager.verify(&[]).await?;
        let profiles = profiles
            .into_iter()
            .map(|profile| (profile.language_id.clone(), profile))
            .collect::<HashMap<_, _>>();
        let mut bundles = HashMap::new();
        for report in reports {
            let profile = profiles.get(&report.language_id).ok_or_else(|| {
                format!(
                    "verified schema bundle has no registered profile: {}",
                    report.language_id
                )
            })?;
            let receipt = load_verified_bundle_receipt(&report.receipt_path)?;
            let schema_root = report.receipt_path.parent().ok_or_else(|| {
                format!(
                    "schema bundle receipt has no parent: {}",
                    report.receipt_path.display()
                )
            })?;
            let mut entries = Vec::with_capacity(receipt.schemas.len());
            let mut documents = Vec::with_capacity(receipt.schemas.len());
            for owned in receipt.schemas {
                let document = read_schema_document(schema_root, &owned.name)?;
                let schema_id = document
                    .get("$id")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| format!("schema document has no $id: {}", owned.name))?
                    .to_owned();
                let family_id = responsibilities.get(&owned.name).ok_or_else(|| {
                    format!(
                        "schema document has no SchemaManager family: {}",
                        owned.name
                    )
                })?;
                let entry = SchemaBundleEntry {
                    family_id: family_id.clone(),
                    schema_id,
                    schema_version: SCHEMA_VERSION.to_owned(),
                    name: owned.name,
                    digest: owned.digest,
                };
                documents.push(SchemaBundleDocument {
                    entry: entry.clone(),
                    document,
                });
                entries.push(entry);
            }
            let projected_receipt = SchemaBundleReceipt {
                language_id: report.language_id.clone(),
                root_set_ids: profile.root_sets.clone(),
                bundle_digest: receipt.bundle_digest,
            };
            let ready = Arc::new(SchemaBundleResponse::Ready {
                schema_id: SCHEMA_BUNDLE_RESPONSE_SCHEMA_ID.to_owned(),
                schema_version: SCHEMA_VERSION.to_owned(),
                receipt: projected_receipt.clone(),
                entries: entries.clone(),
                documents,
            });
            let unchanged = Arc::new(SchemaBundleResponse::Unchanged {
                schema_id: SCHEMA_BUNDLE_RESPONSE_SCHEMA_ID.to_owned(),
                schema_version: SCHEMA_VERSION.to_owned(),
                receipt: projected_receipt.clone(),
                entries,
            });
            let profile_mismatch = Arc::new(failed(
                &report.language_id,
                "schema-bundle-profile-mismatch",
                serde_json::json!({"action": "use-registered-root-set-profile"}),
                serde_json::json!({"registeredRootSetIds": profile.root_sets}),
            ));
            ready.validate()?;
            unchanged.validate()?;
            profile_mismatch.validate()?;
            let projected = VerifiedLanguageBundle {
                root_set_ids: Arc::from(profile.root_sets.clone()),
                bundle_digest: projected_receipt.bundle_digest,
                ready,
                unchanged,
                profile_mismatch,
            };
            if bundles
                .insert(report.language_id.clone(), projected)
                .is_some()
            {
                return Err(format!(
                    "duplicate verified schema bundle: {}",
                    report.language_id
                ));
            }
        }
        Ok(Self {
            bundles: Arc::new(bundles),
        })
    }

    pub fn project(&self, request: &SchemaBundleRequest) -> Arc<SchemaBundleResponse> {
        match self.bundles.get(&request.language_id) {
            None => Arc::new(failed(
                &request.language_id,
                "schema-bundle-language-unregistered",
                serde_json::json!({"action": "select-registered-language-profile"}),
                serde_json::json!({"registeredLanguages": self.registered_languages()}),
            )),
            Some(bundle) if request.root_set_ids.as_slice() != bundle.root_set_ids.as_ref() => {
                Arc::clone(&bundle.profile_mismatch)
            }
            Some(bundle)
                if request.known_bundle_digest.as_deref()
                    == Some(bundle.bundle_digest.as_str()) =>
            {
                Arc::clone(&bundle.unchanged)
            }
            Some(bundle) => Arc::clone(&bundle.ready),
        }
    }

    fn registered_languages(&self) -> Vec<&str> {
        let mut languages = self.bundles.keys().map(String::as_str).collect::<Vec<_>>();
        languages.sort_unstable();
        languages
    }
}

#[cfg(test)]
mod binding_tests {
    use super::RuntimeSchemaBundleCatalog;

    #[test]
    fn embedded_bundle_binding_is_v1_deterministic_and_complete() {
        let catalog = RuntimeSchemaBundleCatalog::load_embedded().expect("embedded schema catalog");
        let once = catalog
            .binding_digest(["rust"])
            .expect("Rust schema bundle binding");
        let duplicated = catalog
            .binding_digest(["rust", "rust"])
            .expect("deduplicated Rust schema bundle binding");
        assert_eq!(once, duplicated);
        assert!(once.starts_with("blake3-256:"));

        let missing = catalog
            .binding_digest(["not-a-registered-language"])
            .expect_err("missing required schema bundle must fail closed");
        assert!(
            missing.contains("absent for required language"),
            "{missing}"
        );
    }
}

fn read_schema_document(schema_root: &Path, name: &str) -> Result<serde_json::Value, String> {
    if name == BUNDLE_RECEIPT_FILE || name.contains('/') {
        return Err(format!("invalid receipt-owned schema name: {name}"));
    }
    let bytes = std::fs::read(schema_root.join(name))
        .map_err(|error| format!("read verified schema document {name}: {error}"))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("decode verified schema document {name}: {error}"))
}

fn failed(
    language_id: &str,
    reason_kind: &str,
    recommended_next: serde_json::Value,
    details: serde_json::Value,
) -> SchemaBundleResponse {
    SchemaBundleResponse::Failed {
        schema_id: SCHEMA_BUNDLE_RESPONSE_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        language_id: language_id.to_owned(),
        reason_kind: reason_kind.to_owned(),
        recommended_next,
        details,
    }
}

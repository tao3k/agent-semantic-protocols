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

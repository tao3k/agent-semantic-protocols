// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Thin Language bootstrap admission against canonical Schema Manager authority.

use std::fs;
use std::path::PathBuf;

use crate::manager::SchemaManager;
use crate::manager_validation::validate_identity;
use crate::manager_validation::validate_schema_name;
use crate::receipt::read_public_receipt_blocking;
use crate::receipt::schema_digest;
use crate::task_owner::run_blocking;

/// Package-local V1 bootstrap projection checked against canonical authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LanguageSchemaBootstrapProjection {
    pub language_id: String,
    pub receipt_path: PathBuf,
    pub schema_name: String,
    pub schema_path: PathBuf,
}

/// Identity proof returned after a bootstrap projection matches canonical bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedLanguageSchemaBootstrap {
    pub language_id: String,
    pub bundle_digest: String,
    pub schema_name: String,
    pub schema_digest: String,
}

impl SchemaManager {
    /// Verify a thin package-local V1 receipt and one bootstrap schema without
    /// materializing or trusting a package-local copy of the shared closure.
    pub async fn verify_client_bootstrap_projection(
        &self,
        projection: LanguageSchemaBootstrapProjection,
    ) -> Result<VerifiedLanguageSchemaBootstrap, String> {
        let manager = self.clone();
        run_blocking("schema-verify-bootstrap-projection", move || {
            manager.verify_client_bootstrap_projection_blocking(&projection)
        })
        .await
    }

    fn verify_client_bootstrap_projection_blocking(
        &self,
        projection: &LanguageSchemaBootstrapProjection,
    ) -> Result<VerifiedLanguageSchemaBootstrap, String> {
        validate_identity("languageId", &projection.language_id)?;
        validate_schema_name(&projection.schema_name)?;
        let registry = self.load_registry()?;
        let requested = vec![projection.language_id.clone()];
        let profile = self
            .select_profiles(&registry, &requested)?
            .into_iter()
            .next()
            .ok_or_else(|| {
                format!(
                    "unknown language schema profile: {}",
                    projection.language_id
                )
            })?;
        let (expected, documents) = self.expected_receipt(&registry, profile)?;
        let public_receipt = read_public_receipt_blocking(&projection.receipt_path)?;
        if public_receipt.schema_digest != expected.bundle_digest {
            return Err(format!(
                "schema bootstrap receipt is stale for {}: expected={} actual={}",
                projection.language_id, expected.bundle_digest, public_receipt.schema_digest
            ));
        }
        let canonical_bytes = documents.get(&projection.schema_name).ok_or_else(|| {
            format!(
                "schema bootstrap is outside the canonical closure for {}: {}",
                projection.language_id, projection.schema_name
            )
        })?;
        let projected_bytes = fs::read(&projection.schema_path).map_err(|error| {
            format!(
                "read schema bootstrap projection {}: {error}",
                projection.schema_path.display()
            )
        })?;
        if &projected_bytes != canonical_bytes {
            return Err(format!(
                "schema bootstrap projection drift for {}: {}",
                projection.language_id, projection.schema_name
            ));
        }
        Ok(VerifiedLanguageSchemaBootstrap {
            language_id: projection.language_id.clone(),
            bundle_digest: expected.bundle_digest,
            schema_name: projection.schema_name.clone(),
            schema_digest: schema_digest(canonical_bytes),
        })
    }
}

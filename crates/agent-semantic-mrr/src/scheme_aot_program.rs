// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::BTreeMap;
use std::fmt;

use serde_json::{Map, Value};

use crate::kernel::ReasoningBundleId;

const COMPILATION_SCHEMA_ID: &str = "agent.semantic-protocols.mrr-program-compilation-receipt";
const SCHEMA_VERSION: &str = "1";

/// Exact Scheme/POO source, native ABI, and admitted MRR bundle tuple exposed
/// by the ahead-of-time compilation boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemeAotProgramBinding {
    scheme_program_digest: String,
    compiled_program_abi_digest: String,
    mrr_bundle_identity: ReasoningBundleId,
}

impl SchemeAotProgramBinding {
    #[must_use]
    pub fn scheme_program_digest(&self) -> &str {
        &self.scheme_program_digest
    }

    #[must_use]
    pub fn compiled_program_abi_digest(&self) -> &str {
        &self.compiled_program_abi_digest
    }

    #[must_use]
    pub const fn mrr_bundle_identity(&self) -> &ReasoningBundleId {
        &self.mrr_bundle_identity
    }
}

/// Admit the compilation receipt before a project-specific owner attaches the
/// program to its own workspace or topology generation.
pub fn admit_scheme_aot_program_binding(
    binding: &Map<String, Value>,
    receipt: &Map<String, Value>,
    admitted_receipts: &BTreeMap<String, Value>,
) -> Result<SchemeAotProgramBinding, SchemeAotProgramBindingError> {
    let encoded_bundle = text(binding, "mrrBundleIdentity")?;
    let mrr_bundle_identity = encoded_bundle
        .parse::<ReasoningBundleId>()
        .map_err(|cause| {
            error(
                "mrr-bundle-identity-invalid",
                format!("mrrBundleIdentity is not a ReasoningBundleId: {cause}"),
            )
        })?;
    require_text(receipt, "schemaId", COMPILATION_SCHEMA_ID)?;
    require_text(receipt, "schemaVersion", SCHEMA_VERSION)?;
    require_text(receipt, "state", "admitted")?;
    let receipt_id = text(receipt, "id")?;
    if admitted_receipts.get(receipt_id) != Some(&Value::Object(receipt.clone())) {
        return Err(error(
            "mrr-compilation-receipt-unadmitted",
            "embedded compilation receipt is not independently admitted",
        ));
    }
    for field in [
        "schemeProgramDigest",
        "compiledProgramAbiDigest",
        "mrrBundleIdentity",
    ] {
        if receipt.get(field) != binding.get(field) {
            return Err(error(
                "mrr-compilation-receipt-mismatch",
                format!("compilation receipt does not bind {field}"),
            ));
        }
    }
    Ok(SchemeAotProgramBinding {
        scheme_program_digest: text(binding, "schemeProgramDigest")?.to_owned(),
        compiled_program_abi_digest: text(binding, "compiledProgramAbiDigest")?.to_owned(),
        mrr_bundle_identity,
    })
}

fn text<'a>(
    value: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, SchemeAotProgramBindingError> {
    value.get(field).and_then(Value::as_str).ok_or_else(|| {
        error(
            "mrr-program-binding-invalid",
            format!("{field} must be a string"),
        )
    })
}

fn require_text(
    value: &Map<String, Value>,
    field: &str,
    expected: &str,
) -> Result<(), SchemeAotProgramBindingError> {
    if text(value, field)? == expected {
        Ok(())
    } else {
        Err(error(
            "mrr-program-binding-invalid",
            format!("{field} does not match {expected}"),
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemeAotProgramBindingError {
    reason_kind: &'static str,
    message: String,
}

impl SchemeAotProgramBindingError {
    #[must_use]
    pub const fn reason_kind(&self) -> &'static str {
        self.reason_kind
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for SchemeAotProgramBindingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.reason_kind, self.message)
    }
}

impl std::error::Error for SchemeAotProgramBindingError {}

fn error(reason_kind: &'static str, message: impl Into<String>) -> SchemeAotProgramBindingError {
    SchemeAotProgramBindingError {
        reason_kind,
        message: message.into(),
    }
}

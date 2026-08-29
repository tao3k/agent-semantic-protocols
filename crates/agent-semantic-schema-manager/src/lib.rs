//! Schema bundle publication and canonical responsibility governance.

pub mod build_support;
mod cli;
mod manager;
mod manager_validation;
mod receipt;
mod responsibility;

pub use agent_semantic_content_identity::{SchemaContractIdentity, schema_contract_identities};
pub use cli::run_cli;
pub use manager::{
    BUNDLE_RECEIPT_FILE, BUNDLE_RECEIPT_SCHEMA_ID, DEFAULT_PROFILE_REGISTRY,
    LanguageSchemaBundleReceipt, LanguageSchemaProfile, LanguageSchemaProfileRegistry,
    PROFILE_REGISTRY_SCHEMA_ID, SCHEMA_VERSION, SchemaBundleEntry, SchemaBundleReport,
    SchemaManager, load_verified_bundle_receipt, verify_bundle_receipt,
};
pub use responsibility::{
    SchemaFamily, SchemaFamilyMembershipOverrides, SchemaFamilyNamespace, SchemaReferenceDecision,
    SchemaResponsibility,
};

#[cfg(test)]
#[path = "../tests/unit/provider_registry.rs"]
mod provider_registry_tests;

#[cfg(test)]
#[path = "../tests/unit/schema_manager.rs"]
mod tests;

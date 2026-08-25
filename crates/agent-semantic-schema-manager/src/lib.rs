//! Schema bundle publication and canonical responsibility governance.

mod cli;
mod manager;
mod responsibility;

pub use cli::run_cli;
pub use manager::{
    BUNDLE_RECEIPT_FILE, BUNDLE_RECEIPT_SCHEMA_ID, DEFAULT_PROFILE_REGISTRY,
    LanguageSchemaBundleReceipt, LanguageSchemaProfile, LanguageSchemaProfileRegistry,
    PROFILE_REGISTRY_SCHEMA_ID, SCHEMA_VERSION, SchemaBundleEntry, SchemaBundleReport,
    SchemaManager, verify_bundle_receipt,
};
pub use responsibility::{
    SchemaFamily, SchemaFamilyMembershipOverrides, SchemaFamilyNamespace, SchemaReferenceDecision,
    SchemaResponsibility,
};

#[cfg(test)]
#[path = "../tests/unit/schema_manager.rs"]
mod tests;

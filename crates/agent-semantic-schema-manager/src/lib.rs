mod cli;
mod manager;

pub use cli::run_cli;
pub use manager::{
    BUNDLE_RECEIPT_FILE, BUNDLE_RECEIPT_SCHEMA_ID, DEFAULT_PROFILE_REGISTRY,
    LanguageSchemaBundleReceipt, LanguageSchemaProfile, LanguageSchemaProfileRegistry,
    PROFILE_REGISTRY_SCHEMA_ID, SCHEMA_VERSION, SchemaBundleEntry, SchemaBundleReport,
    SchemaManager, verify_bundle_receipt,
};

#[cfg(test)]
#[path = "../tests/unit/schema_manager.rs"]
mod tests;

//! Schema bundle publication and canonical responsibility governance.

pub mod build_support;
#[cfg(feature = "runtime")]
mod cli;
#[cfg(feature = "runtime")]
mod manager;
#[cfg(feature = "runtime")]
mod manager_validation;
#[cfg(feature = "runtime")]
mod receipt;
#[cfg(feature = "runtime")]
mod responsibility;
#[cfg(feature = "runtime")]
mod search_architecture_inventory;

#[cfg(feature = "runtime")]
pub use agent_semantic_content_identity::{SchemaContractIdentity, schema_contract_identities};
#[cfg(feature = "runtime")]
pub use cli::run_cli;
#[cfg(feature = "runtime")]
pub use manager::{
    BUNDLE_RECEIPT_FILE, BUNDLE_RECEIPT_SCHEMA_ID, DEFAULT_PROFILE_REGISTRY,
    LanguageSchemaBundleReceipt, LanguageSchemaProfile, LanguageSchemaProfileRegistry,
    PROFILE_REGISTRY_SCHEMA_ID, ResolvedLanguageSchemaBundle, ResolvedSchemaDocument,
    SCHEMA_VERSION, SchemaBundleEntry, SchemaBundleReport, SchemaManager,
    load_verified_bundle_receipt, verify_bundle_receipt,
};
#[cfg(feature = "runtime")]
pub use responsibility::{
    SchemaFamily, SchemaFamilyMembershipOverrides, SchemaFamilyNamespace, SchemaReferenceDecision,
    SchemaResponsibility,
};
#[cfg(feature = "runtime")]
pub use search_architecture_inventory::{
    SEARCH_ARCHITECTURE_INVENTORY_SCHEMA_ID, SEARCH_ARCHITECTURE_INVENTORY_SCHEMA_VERSION,
    SearchArchitectureEdgeKind, SearchArchitectureFactEdge, SearchArchitectureFactInventory,
    SearchArchitectureFactNode, SearchArchitectureInventoryError,
};

#[cfg(all(test, feature = "runtime"))]
#[path = "../tests/unit/provider_registry.rs"]
mod provider_registry_tests;

#[cfg(all(test, feature = "runtime"))]
#[path = "../tests/unit/schema_manager.rs"]
mod tests;

#[cfg(all(test, feature = "runtime"))]
#[path = "../tests/unit/search_architecture_inventory.rs"]
mod search_architecture_inventory_tests;

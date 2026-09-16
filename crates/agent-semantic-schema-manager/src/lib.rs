// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

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
pub use agent_semantic_content_identity::SchemaContractIdentity;
#[cfg(feature = "runtime")]
pub use agent_semantic_content_identity::schema_contract_identities;
#[cfg(feature = "runtime")]
pub use cli::run_cli;
#[cfg(feature = "runtime")]
pub use manager::BUNDLE_RECEIPT_FILE;
#[cfg(feature = "runtime")]
pub use manager::BUNDLE_RECEIPT_SCHEMA_ID;
#[cfg(feature = "runtime")]
pub use manager::DEFAULT_PROFILE_REGISTRY;
#[cfg(feature = "runtime")]
pub use manager::LanguageSchemaBundleReceipt;
#[cfg(feature = "runtime")]
pub use manager::LanguageSchemaProfile;
#[cfg(feature = "runtime")]
pub use manager::LanguageSchemaProfileRegistry;
#[cfg(feature = "runtime")]
pub use manager::PROFILE_REGISTRY_SCHEMA_ID;
#[cfg(feature = "runtime")]
pub use manager::ResolvedLanguageSchemaBundle;
#[cfg(feature = "runtime")]
pub use manager::ResolvedSchemaDocument;
#[cfg(feature = "runtime")]
pub use manager::SCHEMA_VERSION;
#[cfg(feature = "runtime")]
pub use manager::SchemaBundleEntry;
#[cfg(feature = "runtime")]
pub use manager::SchemaBundleReport;
#[cfg(feature = "runtime")]
pub use manager::SchemaManager;
#[cfg(feature = "runtime")]
pub use manager::SearchProducerAxis;
#[cfg(feature = "runtime")]
pub use manager::load_verified_bundle_receipt;
#[cfg(feature = "runtime")]
pub use manager::verify_bundle_receipt;
#[cfg(feature = "runtime")]
pub use responsibility::SchemaFamily;
#[cfg(feature = "runtime")]
pub use responsibility::SchemaFamilyMembershipOverrides;
#[cfg(feature = "runtime")]
pub use responsibility::SchemaFamilyNamespace;
#[cfg(feature = "runtime")]
pub use responsibility::SchemaReferenceDecision;
#[cfg(feature = "runtime")]
pub use responsibility::SchemaResponsibility;
#[cfg(feature = "runtime")]
pub use search_architecture_inventory::SEARCH_ARCHITECTURE_INVENTORY_SCHEMA_ID;
#[cfg(feature = "runtime")]
pub use search_architecture_inventory::SEARCH_ARCHITECTURE_INVENTORY_SCHEMA_VERSION;
#[cfg(feature = "runtime")]
pub use search_architecture_inventory::SearchArchitectureEdgeKind;
#[cfg(feature = "runtime")]
pub use search_architecture_inventory::SearchArchitectureFactEdge;
#[cfg(feature = "runtime")]
pub use search_architecture_inventory::SearchArchitectureFactInventory;
#[cfg(feature = "runtime")]
pub use search_architecture_inventory::SearchArchitectureFactNode;
#[cfg(feature = "runtime")]
pub use search_architecture_inventory::SearchArchitectureInventoryError;

#[cfg(all(test, feature = "runtime"))]
#[path = "../tests/unit/provider_registry.rs"]
mod provider_registry_tests;

#[cfg(all(test, feature = "runtime"))]
#[path = "../tests/unit/schema_manager.rs"]
mod tests;

#[cfg(all(test, feature = "runtime"))]
#[path = "../tests/unit/search_architecture_inventory.rs"]
mod search_architecture_inventory_tests;

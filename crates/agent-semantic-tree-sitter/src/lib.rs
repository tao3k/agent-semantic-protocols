#![deny(dead_code)]

//! Shared tree-sitter-compatible query catalog utilities for ASP.
//!
//! This crate owns ASP-side catalog loading, fingerprinting, and runtime query
//! compilation. Language providers keep native parser authority and maintain
//! `.scm` catalogs; they do not need to link tree-sitter runtime or grammar
//! crates for this ABI.

pub mod builtin_catalog;
pub mod catalog;
mod language_registry;
pub mod query_syntax;
pub mod runtime;

pub use agent_semantic_tree_sitter_runtime::{
    CompiledSyntaxQuery as CompiledNativeSyntaxQuery, NativeQueryCapture, NativeQueryExecution,
    NativeQueryMatch, NativeQueryNode, compile_query_source as compile_native_query_source,
    execute_query as execute_native_query,
};
pub use builtin_catalog::{BuiltinCatalogId, BuiltinCatalogLanguageId, builtin_catalog_source};
pub use catalog::{
    LoadedGrammarProfile, LoadedSyntaxCatalog, SyntaxCatalogDescriptor, extract_capture_names,
    fingerprint_catalog, fingerprint_grammar_profile, load_grammar_profile, load_syntax_catalog,
    normalize_capture_names,
};
pub use language_registry::registered_language_grammar;

#[cfg(test)]
#[path = "../tests/unit/workspace_runtime.rs"]
mod workspace_runtime_tests;
pub use query_syntax::{
    SyntaxQueryAbiError, SyntaxQueryAbiPattern, SyntaxQueryAbiPlan, SyntaxQueryAbiPredicate,
    SyntaxQueryPredicateOp, SyntaxQueryPredicateValue, compile_query_abi_source,
};
pub use runtime::{
    CompiledSyntaxQuery, SyntaxQueryCompileError, compile_catalog_query, compile_query_source,
};

#[cfg(test)]
#[path = "../tests/unit/catalog.rs"]
mod catalog_tests;
#[cfg(test)]
#[path = "../tests/unit/query_syntax.rs"]
mod query_syntax_tests;

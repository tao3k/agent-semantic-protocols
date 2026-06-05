//! Shared tree-sitter-compatible query catalog utilities for ASP.
//!
//! This crate owns ASP-side catalog loading, fingerprinting, and runtime query
//! compilation. Language providers keep native parser authority and maintain
//! `.scm` catalogs; they do not need to link tree-sitter runtime or grammar
//! crates for this ABI.

pub mod catalog;
pub mod query_syntax;
pub mod runtime;

pub use catalog::{
    LoadedGrammarProfile, LoadedSyntaxCatalog, SyntaxCatalogDescriptor, extract_capture_names,
    fingerprint_catalog, fingerprint_grammar_profile, load_grammar_profile, load_syntax_catalog,
    normalize_capture_names,
};
pub use query_syntax::{
    SyntaxQueryAbiError, SyntaxQueryAbiPattern, SyntaxQueryAbiPlan, compile_query_abi_source,
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

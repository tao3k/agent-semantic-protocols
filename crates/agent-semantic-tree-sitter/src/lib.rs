#![deny(dead_code)]

//! Shared S-expression syntax-query catalog utilities for ASP.
//!
//! This crate only loads `.scm` catalogs and compiles their Tree-sitter-compatible
//! S-expression surface into a grammarless typed plan. Language providers retain
//! parser authority and execute that plan against richer native syntax facts.

pub mod builtin_catalog;
pub mod catalog;
pub mod query_syntax;
pub use builtin_catalog::{BuiltinCatalogId, BuiltinCatalogLanguageId, builtin_catalog_source};
pub use catalog::{
    LoadedGrammarProfile, LoadedSyntaxCatalog, SyntaxCatalogDescriptor, extract_capture_names,
    fingerprint_catalog, fingerprint_grammar_profile, load_grammar_profile, load_syntax_catalog,
    normalize_capture_names,
};
pub use query_syntax::{
    SyntaxQueryAbiError, SyntaxQueryAbiPattern, SyntaxQueryAbiPlan, SyntaxQueryAbiPredicate,
    SyntaxQueryPredicateOp, SyntaxQueryPredicateValue, compile_query_abi_source,
};

#[cfg(test)]
#[path = "../tests/unit/catalog.rs"]
mod catalog_tests;
#[cfg(test)]
#[path = "../tests/unit/query_syntax.rs"]
mod query_syntax_tests;

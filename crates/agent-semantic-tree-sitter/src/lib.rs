#![deny(dead_code)]

//! Shared S-expression syntax-query catalog utilities for ASP.
//!
//! This crate only loads `.scm` catalogs and compiles their Tree-sitter-compatible
//! S-expression surface into a grammarless typed plan. Language providers retain
//! parser authority and execute that plan against richer native syntax facts.

pub mod builtin_catalog;
pub mod catalog;
pub mod query_syntax;
pub use builtin_catalog::BuiltinCatalogId;
pub use builtin_catalog::BuiltinCatalogLanguageId;
pub use builtin_catalog::builtin_catalog_source;
pub use catalog::LoadedGrammarProfile;
pub use catalog::LoadedSyntaxCatalog;
pub use catalog::SyntaxCatalogDescriptor;
pub use catalog::extract_capture_names;
pub use catalog::fingerprint_catalog;
pub use catalog::fingerprint_grammar_profile;
pub use catalog::load_grammar_profile;
pub use catalog::load_syntax_catalog;
pub use catalog::normalize_capture_names;
pub use query_syntax::SyntaxQueryAbiError;
pub use query_syntax::SyntaxQueryAbiPattern;
pub use query_syntax::SyntaxQueryAbiPlan;
pub use query_syntax::SyntaxQueryAbiPredicate;
pub use query_syntax::SyntaxQueryPredicateOp;
pub use query_syntax::SyntaxQueryPredicateValue;
pub use query_syntax::compile_query_abi_source;

#[cfg(test)]
#[path = "../tests/unit/catalog.rs"]
mod catalog_tests;
#[cfg(test)]
#[path = "../tests/unit/query_syntax.rs"]
mod query_syntax_tests;

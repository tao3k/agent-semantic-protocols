#![deny(dead_code)]
// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Shared S-expression syntax-query catalog utilities for ASP.
//!
//! This crate only loads `.scm` catalogs and compiles their Tree-sitter-compatible
//! S-expression surface through the official Query grammar. Language providers
//! retain parser authority and execute that compatibility projection against
//! richer native syntax facts.

pub mod builtin_catalog;
pub mod catalog;
pub mod enhanced_query;
pub mod query_syntax;
mod resident_syntax_plan;
pub mod scheme_source;
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
pub use enhanced_query::{
    EnhancedQueryDocument, EnhancedQueryExpression, EnhancedQueryOperand, EnhancedQueryParseError,
    EnhancedQueryPattern, EnhancedQueryPredicate, EnhancedQueryPredicateKind,
    EnhancedQueryQuantifier, parse_enhanced_query_source,
};
pub use query_syntax::SyntaxQueryAbiError;
pub use query_syntax::SyntaxQueryAbiPattern;
pub use query_syntax::SyntaxQueryAbiPlan;
pub use query_syntax::SyntaxQueryAbiPredicate;
pub use query_syntax::SyntaxQueryPredicateOp;
pub use query_syntax::SyntaxQueryPredicateValue;
pub use query_syntax::compile_query_abi_source;
pub use resident_syntax_plan::compile_resident_syntax_plan;
pub use scheme_source::{
    SCHEME_GRAMMAR_ID, SCHEME_GRAMMAR_REPOSITORY, SCHEME_GRAMMAR_VERSION, SchemeDatum,
    SchemeSourceAdmission, SchemeSourceAdmissionError, admit_scheme_source, parse_scheme_datums,
};

#[cfg(test)]
#[path = "../tests/unit/catalog.rs"]
mod catalog_tests;
#[cfg(test)]
#[path = "../tests/unit/enhanced_query.rs"]
mod enhanced_query_tests;
#[cfg(test)]
#[path = "../tests/unit/query_syntax.rs"]
mod query_syntax_tests;
#[cfg(test)]
#[path = "../tests/unit/scheme_source.rs"]
mod scheme_source_tests;

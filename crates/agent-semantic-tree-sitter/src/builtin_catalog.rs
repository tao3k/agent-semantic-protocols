// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Built-in tree-sitter-compatible query catalogs shipped inside the ASP binary.

/// Public language id used to resolve an embedded tree-sitter query catalog.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BuiltinCatalogLanguageId<'a>(&'a str);

impl<'a> BuiltinCatalogLanguageId<'a> {
    /// Return the wire language id.
    #[must_use]
    pub const fn as_str(self) -> &'a str {
        self.0
    }
}

impl<'a> From<&'a str> for BuiltinCatalogLanguageId<'a> {
    fn from(value: &'a str) -> Self {
        Self(value)
    }
}

/// Public catalog id used to resolve an embedded tree-sitter query catalog.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BuiltinCatalogId<'a>(&'a str);

impl<'a> BuiltinCatalogId<'a> {
    /// Return the wire catalog id.
    #[must_use]
    pub const fn as_str(self) -> &'a str {
        self.0
    }
}

impl<'a> From<&'a str> for BuiltinCatalogId<'a> {
    fn from(value: &'a str) -> Self {
        Self(value)
    }
}

/// Embedded catalog source resolved by public language id and catalog id.
///
/// These sources are used by the ASP wrap layer to compile the query ABI plan
/// before a provider native projection runs. Providers can still embed the same
/// `.scm` files for direct debug execution, but the public `asp <language>
/// query --catalog ...` path does not depend on provider package source being
/// present at runtime.
#[must_use]
pub fn builtin_catalog_source(
    language_id: BuiltinCatalogLanguageId<'_>,
    catalog_id: BuiltinCatalogId<'_>,
) -> Option<&'static str> {
    let catalog_id = catalog_id.as_str();
    match language_id.as_str() {
        "rust" => rust_catalog_source(catalog_id),
        "typescript" => typescript_catalog_source(catalog_id),
        "python" => python_catalog_source(catalog_id),
        "julia" => julia_catalog_source(catalog_id),
        _ => None,
    }
}

fn rust_catalog_source(catalog_id: &str) -> Option<&'static str> {
    match catalog_id {
        "calls" => Some(include_str!("../queries/rust/calls.scm")),
        "cfg" => Some(include_str!("../queries/rust/cfg.scm")),
        "declarations" => Some(include_str!("../queries/rust/declarations.scm")),
        "imports" => Some(include_str!("../queries/rust/imports.scm")),
        "macros" => Some(include_str!("../queries/rust/macros.scm")),
        _ => None,
    }
}

fn typescript_catalog_source(catalog_id: &str) -> Option<&'static str> {
    match catalog_id {
        "calls" => Some(include_str!("../queries/typescript/calls.scm")),
        "declarations" => Some(include_str!("../queries/typescript/declarations.scm")),
        "imports" => Some(include_str!("../queries/typescript/imports.scm")),
        _ => None,
    }
}

fn python_catalog_source(catalog_id: &str) -> Option<&'static str> {
    match catalog_id {
        "calls" => Some(include_str!("../queries/python/calls.scm")),
        "control-flow" => Some(include_str!("../queries/python/control-flow.scm")),
        "declarations" => Some(include_str!("../queries/python/declarations.scm")),
        "decorators" => Some(include_str!("../queries/python/decorators.scm")),
        "imports" => Some(include_str!("../queries/python/imports.scm")),
        _ => None,
    }
}

fn julia_catalog_source(catalog_id: &str) -> Option<&'static str> {
    match catalog_id {
        "calls" => Some(include_str!("../queries/julia/calls.scm")),
        "declarations" => Some(include_str!("../queries/julia/declarations.scm")),
        "imports" => Some(include_str!("../queries/julia/imports.scm")),
        _ => None,
    }
}

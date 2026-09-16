// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Lightweight canonical Tree-sitter Scheme source admission.

mod scheme_syntax;

pub use scheme_syntax::{
    SCHEME_GRAMMAR_ID, SCHEME_GRAMMAR_REPOSITORY, SCHEME_GRAMMAR_VERSION, SchemeDatum,
    SchemeSourceAdmission, SchemeSourceAdmissionError, admit_scheme_source, parse_scheme_datums,
};

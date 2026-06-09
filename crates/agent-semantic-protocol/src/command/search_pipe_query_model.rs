//! Shared query-pack data model.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TermRole {
    Context,
    Concept,
    Symbol,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct QueryTerm {
    pub(super) raw: String,
    pub(super) lower: String,
    pub(super) role: TermRole,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct QueryClause {
    pub(super) terms: Vec<QueryTerm>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ClauseCoverage {
    pub(super) id: usize,
    pub(super) matched: Vec<String>,
    pub(super) missing: Vec<String>,
}

impl TermRole {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Context => "context",
            Self::Concept => "concept",
            Self::Symbol => "symbol",
        }
    }
}

//! Shared search-pipe data model.

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Candidate {
    pub(super) path: String,
    pub(super) line: usize,
    pub(super) symbol: String,
    pub(super) text: String,
    pub(super) source: String,
    pub(super) confidence: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SearchPipeSourceTrace {
    pub(super) source: String,
    pub(super) status: String,
    pub(super) matched: usize,
    pub(super) missing: usize,
    pub(super) normalized: usize,
}

impl SearchPipeSourceTrace {
    pub(super) fn new(
        source: impl Into<String>,
        status: impl Into<String>,
        matched: usize,
        missing: usize,
        normalized: usize,
    ) -> Self {
        Self {
            source: source.into(),
            status: status.into(),
            matched,
            missing,
            normalized,
        }
    }

    pub(super) fn compact(&self) -> String {
        format!("{}:{}", self.source, self.status)
    }
}

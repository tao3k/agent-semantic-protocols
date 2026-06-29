//! Query-wrapper shared data model.

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FdQueryPreview {
    pub(super) owner_candidates: Vec<String>,
    pub(super) package_clusters: Vec<String>,
    pub(super) rg_scope_next: Vec<String>,
}

impl FdQueryPreview {
    pub(super) fn is_empty(&self) -> bool {
        self.owner_candidates.is_empty()
            && self.package_clusters.is_empty()
            && self.rg_scope_next.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum QueryWrapperSurface {
    Fd,
    Rg,
}

impl QueryWrapperSurface {
    pub(super) fn from_command(command: &str) -> Option<Self> {
        match command {
            "fd" => Some(Self::Fd),
            "rg" => Some(Self::Rg),
            _ => None,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Fd => "fd",
            Self::Rg => "rg",
        }
    }

    pub(super) fn graph_surface(self) -> &'static str {
        match self {
            Self::Fd => "search-fd",
            Self::Rg => "search-rg",
        }
    }

    pub(super) fn source_name(self) -> &'static str {
        "finder"
    }

    pub(super) fn next_classes(self, quality: &QueryWrapperQuality) -> &'static str {
        if !quality.allow_query_selector {
            return match self {
                Self::Fd => "owner-items,scoped-rg-query,rg-query",
                Self::Rg => "fd-query,scoped-rg-query,owner-items",
            };
        }
        match self {
            Self::Fd => "owner-items,rg-query,query-selector",
            Self::Rg => "owner-items,query-selector,fd-query",
        }
    }

    pub(super) fn avoid(self, quality: &QueryWrapperQuality) -> &'static str {
        if !quality.allow_query_selector {
            return match self {
                Self::Fd => "repeat-flat-fd,workspace-wide-fd,raw-read",
                Self::Rg => "repeat-flat-rg,workspace-wide-rg,manual-window-scan,raw-read",
            };
        }
        match self {
            Self::Fd => "repeat-fd,raw-read",
            Self::Rg => "repeat-rg,manual-window-scan,raw-read",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct QueryWrapperClause {
    pub(super) id: usize,
    pub(super) raw: String,
    pub(super) terms: Vec<String>,
    pub(super) axis_terms: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct QueryWrapperClauseCoverage {
    pub(super) id: usize,
    pub(super) matched: Vec<String>,
    pub(super) missing: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct QueryWrapperQuality {
    pub(super) query_pack_quality: String,
    pub(super) scope_quality: String,
    pub(super) package_cohesion: String,
    pub(super) packages: Vec<String>,
    pub(super) risks: Vec<String>,
    pub(super) noise: Vec<String>,
    pub(super) allow_query_selector: bool,
    pub(super) clause_coverages: Vec<QueryWrapperClauseCoverage>,
}

pub(super) fn display_terms(terms: &[String]) -> String {
    if terms.is_empty() {
        "-".to_string()
    } else {
        terms.join(",")
    }
}

pub(super) fn shell_arg(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

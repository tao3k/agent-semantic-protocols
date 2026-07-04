use std::path::Path;

use agent_semantic_client_db::{ClientDbSourceIndexLookupResult, ClientDbSourceIndexLookupState};

use crate::source_index_lookup::lookup_source_index;

pub struct OwnerItemsSourceIndexTrace {
    pub line: String,
    state: Option<ClientDbSourceIndexLookupState>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OwnerItemsSourceIndexTraceStream {
    Stdout,
    Stderr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnerItemsSourceIndexTraceRender {
    pub line: String,
    pub stream: OwnerItemsSourceIndexTraceStream,
}

impl OwnerItemsSourceIndexTrace {
    #[must_use]
    pub fn new(line: String, state: Option<ClientDbSourceIndexLookupState>) -> Self {
        Self { line, state }
    }

    #[must_use]
    pub fn is_hit(&self) -> bool {
        matches!(self.state, Some(ClientDbSourceIndexLookupState::Hit))
    }

    #[must_use]
    pub fn render(self) -> OwnerItemsSourceIndexTraceRender {
        let stream = if self.is_hit()
            || matches!(
                self.state,
                Some(
                    ClientDbSourceIndexLookupState::MissingDb
                        | ClientDbSourceIndexLookupState::EmptyIndex
                        | ClientDbSourceIndexLookupState::Busy
                )
            ) {
            OwnerItemsSourceIndexTraceStream::Stdout
        } else {
            OwnerItemsSourceIndexTraceStream::Stderr
        };
        OwnerItemsSourceIndexTraceRender {
            line: self.line,
            stream,
        }
    }
}

#[must_use]
pub fn source_index_owner_query(project_root: &Path, owner: &Path) -> String {
    let owner = owner.strip_prefix(project_root).unwrap_or(owner);
    owner.to_string_lossy().replace('\\', "/")
}

pub fn render_owner_items_source_index_trace(
    project_root: &Path,
    owner: &Path,
) -> Result<Option<String>, String> {
    Ok(owner_items_source_index_trace(project_root, owner)?.map(|trace| trace.line))
}

pub fn owner_items_source_index_trace(
    project_root: &Path,
    owner: &Path,
) -> Result<Option<OwnerItemsSourceIndexTrace>, String> {
    let query = source_index_owner_query(project_root, owner);
    let lookup = match lookup_source_index(project_root, &query, 8) {
        Ok(lookup) => lookup,
        Err(error) => {
            return Ok(Some(OwnerItemsSourceIndexTrace::new(
                format!(
                    "|sourceIndex status=error source=source-index query={} reason={}",
                    trace_value(&query),
                    trace_value(&error)
                ),
                None,
            )));
        }
    };
    let state = lookup.state.clone();
    Ok(Some(OwnerItemsSourceIndexTrace::new(
        render_owner_items_source_index_lookup_trace(&query, &lookup),
        Some(state),
    )))
}

#[must_use]
pub fn render_owner_items_source_index_lookup_trace(
    query: &str,
    lookup: &ClientDbSourceIndexLookupResult,
) -> String {
    let mut line = format!(
        "|sourceIndex status={} source=source-index query={}",
        lookup.state.as_str(),
        trace_value(&query)
    );
    match lookup.state {
        ClientDbSourceIndexLookupState::Hit => {
            if let Some(candidate) = lookup.candidates.first() {
                line.push_str(&format!(" path={}", trace_value(&candidate.path)));
            }
        }
        ClientDbSourceIndexLookupState::MissingDb => {
            line.push_str(" reason=sourceIndex:missing-db next=asp_cache_source-index_refresh");
        }
        ClientDbSourceIndexLookupState::EmptyIndex => {
            line.push_str(" reason=sourceIndex:empty-index next=asp_cache_source-index_refresh");
        }
        ClientDbSourceIndexLookupState::Busy => {
            line.push_str(" reason=sourceIndex:busy next=retry_source-index_lookup");
        }
        ClientDbSourceIndexLookupState::Miss => {
            line.push_str(" reason=sourceIndex:miss");
        }
    }
    line
}

fn trace_value(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        "-".to_string()
    } else {
        value.split_whitespace().collect::<Vec<_>>().join("_")
    }
}

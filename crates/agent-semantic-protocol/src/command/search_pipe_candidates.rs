//! Candidate collection for ASP-owned cheap search frontiers.

use std::io::{self, IsTerminal, Read};
use std::path::Path;

use agent_semantic_search::collect_ingest_search_candidates;

use super::search_pipe_model::Candidate;

pub(super) const PIPE_CANDIDATE_LINE_LIMIT: usize = 256;

pub(super) fn read_piped_stdin() -> Result<Vec<u8>, String> {
    let stdin = io::stdin();
    if stdin.is_terminal() {
        return Ok(Vec::new());
    }
    let mut bytes = Vec::new();
    stdin
        .lock()
        .read_to_end(&mut bytes)
        .map_err(|error| format!("failed to read search ingest stdin: {error}"))?;
    Ok(bytes)
}

pub(super) fn parse_ingest_candidates(
    project_root: &Path,
    locator_root: &Path,
    stdin: &[u8],
) -> Vec<Candidate> {
    collect_ingest_search_candidates(project_root, locator_root, stdin, PIPE_CANDIDATE_LINE_LIMIT)
        .into_iter()
        .map(Candidate::from)
        .collect()
}

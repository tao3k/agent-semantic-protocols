//! Dynamic search candidate projection.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use crate::{
    LexicalOverlayDocument, dynamic_overlay::SEARCH_OVERLAY_ROUTE_SOURCE,
    search_lexical_overlay_candidates,
};

mod byte_text {
    pub(super) fn find_byte(needle: u8, haystack: &[u8]) -> Option<usize> {
        memchr::memchr(needle, haystack)
    }

    pub(super) fn lossy_string(bytes: &[u8]) -> String {
        String::from_utf8_lossy(bytes).into_owned()
    }

    pub(super) fn split_lf_or_nul_records(bytes: &[u8]) -> impl Iterator<Item = &[u8]> {
        bytes
            .split(|byte| matches!(*byte, b'\n' | b'\0'))
            .map(trim_ascii)
    }

    fn trim_ascii(bytes: &[u8]) -> &[u8] {
        let start = bytes
            .iter()
            .position(|byte| !byte.is_ascii_whitespace())
            .unwrap_or(bytes.len());
        let end = bytes
            .iter()
            .rposition(|byte| !byte.is_ascii_whitespace())
            .map_or(start, |index| index + 1);
        &bytes[start..end]
    }
}

const DYNAMIC_LEXICAL_OVERLAY_DOCUMENT_SCAN_LIMIT: usize = 256;

/// Candidate projected from a high-churn dynamic search overlay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicSearchCandidate {
    /// Owner path relative to the caller's locator root when possible.
    pub path: String,
    /// Display start line. Dynamic overlay candidates do not make line ranges
    /// executable identity.
    pub line: usize,
    /// Display end line.
    pub end_line: usize,
    /// Symbol or query term that led to the candidate.
    pub symbol: String,
    /// Candidate text used by downstream renderers.
    pub text: String,
    /// Candidate source label.
    pub source: String,
    /// Candidate confidence label.
    pub confidence: String,
}

/// Dynamic candidates bound to the canonical workspace snapshot searched.
#[derive(Debug, Clone)]
pub struct DynamicSearchCandidateCollection {
    /// Snapshot evidence retained from lexical overlay projection.
    pub source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
    /// Compact candidates projected from the same snapshot.
    pub candidates: Vec<DynamicSearchCandidate>,
}

/// Candidate projected from a pipe ingest stream before protocol rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestSearchCandidate {
    /// Owner path relative to the caller's locator root when possible.
    pub path: String,
    /// Display start line. Ingest line ranges remain display metadata.
    pub line: usize,
    /// Display end line.
    pub end_line: usize,
    /// Symbol or text token derived from the ingest record.
    pub symbol: String,
    /// Candidate text used by downstream renderers.
    pub text: String,
    /// Candidate source label.
    pub source: String,
    /// Candidate confidence label.
    pub confidence: String,
}

/// Request for projecting lexical overlay candidates from selected roots.
struct DynamicSearchCandidateRequest<'a> {
    /// Root used for display path normalization.
    pub locator_root: &'a Path,
    /// Query terms normalized by the command/parser layer.
    pub terms: &'a [String],
    /// Search roots whose paths were selected by the caller.
    pub search_roots: &'a [Vec<CommittedDynamicOwner>],
    /// Canonical workspace snapshot selected before language candidate filtering.
    pub source_snapshot: &'a agent_semantic_content_identity::SourceSnapshotEvidence,
    /// Maximum candidates returned.
    pub limit: usize,
}

/// Request for dynamic overlay candidate collection from roots.
pub struct DynamicSearchRootCandidateRequest<'a> {
    /// Root used to resolve relative owner inputs.
    pub project_root: &'a Path,
    /// Root used for display path normalization.
    pub locator_root: &'a Path,
    /// Query terms normalized by the caller.
    pub terms: &'a [String],
    /// Owner roots selected by the caller. Empty means `project_root`.
    pub owners: &'a [PathBuf],
    /// Ignored directory names from the search config.
    pub ignore_dirs: &'a [String],
    /// Hidden directory names that should still be walked.
    pub include_hidden_dirs: &'a [String],
    /// Canonical workspace snapshot selected by the caller.
    pub base_snapshot: &'a agent_semantic_content_identity::WorkspaceSnapshot,
    /// Digest of the selected provider that owns this projection.
    pub provider_digest: &'a str,
    /// Language/provider-owned file predicate.
    pub file_matches: &'a dyn Fn(&Path) -> bool,
    /// Maximum candidates returned.
    pub limit: usize,
}

const RG_COVERAGE_MAX_INPUT_BYTES: usize = 8 * 1024 * 1024;
const RG_COVERAGE_MAX_RECORDS: usize = 4_096;
const RG_COVERAGE_MAX_CANDIDATES: usize = 256;

/// Explicit resource budget for a caller-supplied ripgrep coverage stream.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RgCoverageBudget {
    pub max_input_bytes: usize,
    pub max_records: usize,
    pub max_candidates: usize,
}

impl RgCoverageBudget {
    pub fn new(
        max_input_bytes: usize,
        max_records: usize,
        max_candidates: usize,
    ) -> Result<Self, String> {
        if max_input_bytes == 0
            || max_input_bytes > RG_COVERAGE_MAX_INPUT_BYTES
            || max_records == 0
            || max_records > RG_COVERAGE_MAX_RECORDS
            || max_candidates == 0
            || max_candidates > RG_COVERAGE_MAX_CANDIDATES
        {
            return Err("ripgrep coverage budget is outside the bounded v1 envelope".to_owned());
        }
        Ok(Self {
            max_input_bytes,
            max_records,
            max_candidates,
        })
    }
}

/// Ripgrep coverage output bound to one admitted source generation.
pub struct RgCoverageRequest<'a> {
    pub locator_root: &'a Path,
    pub output: &'a [u8],
    pub generation_digest: &'a str,
    pub source_snapshot: &'a agent_semantic_content_identity::SourceSnapshotEvidence,
    pub workspace_snapshot: &'a agent_semantic_content_identity::WorkspaceSnapshot,
    pub owners: &'a [RgCoverageOwner<'a>],
    pub budget: RgCoverageBudget,
}

/// One immutable owner blob admitted by the workspace generation authority.
pub struct RgCoverageOwner<'a> {
    pub owner_path: &'a str,
    pub content_digest: &'a str,
    pub source: &'a [u8],
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RgCoverageReceipt {
    pub schema_id: &'static str,
    pub schema_version: &'static str,
    pub state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason_kind: Option<&'static str>,
    pub generation_digest: String,
    pub source_root_digest: String,
    pub provider_digest: String,
    pub index_artifact_digest: String,
    pub coverage_input_digest: String,
    pub committed_owner_count: usize,
    pub input_bytes: usize,
    pub record_count: usize,
    pub candidate_count: usize,
}

#[derive(Clone, Debug)]
pub struct RgCoverageResult {
    pub receipt: RgCoverageReceipt,
    pub candidates: Vec<IngestSearchCandidate>,
}

/// Consume explicit `rg --vimgrep --null`-compatible output without spawning a
/// process or falling back from resident lexical search.
pub fn collect_rg_coverage_candidates(
    request: RgCoverageRequest<'_>,
) -> Result<RgCoverageResult, RgCoverageReceipt> {
    let coverage_input_digest = rg_coverage_input_digest(&request);
    let make_receipt = |state: &'static str,
                        reason_kind: Option<&'static str>,
                        record_count,
                        candidate_count| RgCoverageReceipt {
        schema_id: "agent.semantic-protocols.rg-coverage-receipt",
        schema_version: "1",
        state,
        reason_kind,
        generation_digest: request.generation_digest.to_owned(),
        source_root_digest: request.source_snapshot.root_digest.clone(),
        provider_digest: request.source_snapshot.provider_digest.clone(),
        index_artifact_digest: agent_semantic_search_projection::source_index_artifact_digest(
            request.source_snapshot,
        ),
        coverage_input_digest: coverage_input_digest.clone(),
        committed_owner_count: request.owners.len(),
        input_bytes: request.output.len(),
        record_count,
        candidate_count,
    };
    if request.output.len() > request.budget.max_input_bytes {
        return Err(make_receipt("failed", Some("input-budget-exceeded"), 0, 0));
    }
    let records = byte_text::split_lf_or_nul_records(request.output).collect::<Vec<_>>();
    if records.len() > request.budget.max_records {
        return Err(make_receipt(
            "failed",
            Some("record-budget-exceeded"),
            records.len(),
            0,
        ));
    }
    if request.generation_digest.is_empty()
        || request.owners.is_empty()
        || request.source_snapshot.root_digest != request.workspace_snapshot.root_digest()
    {
        return Err(make_receipt(
            "failed",
            Some("owner-set-invalid"),
            records.len(),
            0,
        ));
    }
    let mut committed = BTreeMap::new();
    for owner in request.owners {
        if owner.owner_path.is_empty() || committed.contains_key(owner.owner_path) {
            return Err(make_receipt(
                "failed",
                Some("owner-set-invalid"),
                records.len(),
                0,
            ));
        }
        let Some(expected) = request.workspace_snapshot.file_digest(owner.owner_path) else {
            return Err(make_receipt(
                "failed",
                Some("owner-not-admitted"),
                records.len(),
                0,
            ));
        };
        let observed =
            agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1(
                owner.source,
            );
        if expected != owner.content_digest || observed.as_str() != owner.content_digest {
            return Err(make_receipt(
                "failed",
                Some("owner-content-mismatch"),
                records.len(),
                0,
            ));
        }
        committed.insert(owner.owner_path, owner);
    }
    let mut candidates = Vec::new();
    for line in records.iter().filter(|line| !line.is_empty()) {
        let Some(parsed) = parse_rg_candidate_line(request.locator_root, line) else {
            return Err(make_receipt(
                "failed",
                Some("record-content-mismatch"),
                records.len(),
                0,
            ));
        };
        let Some(owner) = committed.get(parsed.candidate.path.as_str()) else {
            return Err(make_receipt(
                "failed",
                Some("owner-not-admitted"),
                records.len(),
                0,
            ));
        };
        if !owner_line_matches(owner.source, parsed.candidate.line, &parsed.text) {
            return Err(make_receipt(
                "failed",
                Some("record-content-mismatch"),
                records.len(),
                0,
            ));
        }
        if candidates.len() == request.budget.max_candidates {
            return Err(make_receipt(
                "failed",
                Some("candidate-budget-exceeded"),
                records.len(),
                0,
            ));
        }
        candidates.push(parsed.candidate);
    }
    Ok(RgCoverageResult {
        receipt: make_receipt("ready", None, records.len(), candidates.len()),
        candidates,
    })
}

/// Collect dynamic overlay candidates from owner roots.
///
/// The caller supplies explicit owner files. Every file is read once and its
/// bytes must match the canonical workspace snapshot before projection.
pub fn collect_dynamic_lexical_overlay_candidates_from_roots(
    request: DynamicSearchRootCandidateRequest<'_>,
) -> Result<DynamicSearchCandidateCollection, String> {
    let source_snapshot = request.base_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        request.provider_digest,
    );
    let roots = resolved_owner_roots(request.project_root, request.owners);
    let search_roots = roots
        .iter()
        .map(|root| sorted_search_root_files(root, &request))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(collect_dynamic_lexical_overlay_candidates(
        DynamicSearchCandidateRequest {
            locator_root: request.locator_root,
            terms: request.terms,
            search_roots: &search_roots,
            source_snapshot: &source_snapshot,
            limit: request.limit,
        },
    ))
}

struct ParsedRgCandidate {
    candidate: IngestSearchCandidate,
    text: Vec<u8>,
}

fn parse_rg_candidate_line(locator_root: &Path, line: &[u8]) -> Option<ParsedRgCandidate> {
    let path_end = byte_text::find_byte(b':', line)?;
    let raw_path = &line[..path_end];
    let rest = &line[path_end + 1..];
    let line_end = byte_text::find_byte(b':', rest)?;
    let line_number = parse_usize_ascii(&rest[..line_end])?;
    let rest = &rest[line_end + 1..];
    let text = if let Some(column_end) = byte_text::find_byte(b':', rest) {
        if parse_usize_ascii(&rest[..column_end]).is_some() {
            &rest[column_end + 1..]
        } else {
            rest
        }
    } else {
        rest
    };
    let path = PathBuf::from(byte_text::lossy_string(raw_path));
    let absolute = if path.is_absolute() {
        path
    } else {
        locator_root.join(path)
    };
    Some(ParsedRgCandidate {
        text: text.to_vec(),
        candidate: IngestSearchCandidate {
            path: display_path(locator_root, &absolute),
            line: line_number,
            end_line: line_number,
            symbol: symbol_from_bytes(text),
            text: byte_text::lossy_string(text),
            source: "rg-query".to_string(),
            confidence: "likely".to_string(),
        },
    })
}

/// Project path and file-content lexical overlay evidence into compact candidates.
#[must_use]
fn collect_dynamic_lexical_overlay_candidates(
    request: DynamicSearchCandidateRequest<'_>,
) -> DynamicSearchCandidateCollection {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    let documents = request
        .search_roots
        .iter()
        .flat_map(|owners| owners.iter())
        .take(DYNAMIC_LEXICAL_OVERLAY_DOCUMENT_SCAN_LIMIT)
        .map(|owner| lexical_overlay_document(request.locator_root, owner))
        .collect::<Vec<_>>();

    let mut remaining = request.limit;
    let per_term_limit = per_term_candidate_limit(request.terms.len(), request.limit);
    for owners in request.search_roots {
        if remaining == 0 {
            break;
        }
        append_overlay_path_candidates(
            request.locator_root,
            request.terms,
            per_term_limit,
            owners,
            &mut remaining,
            &mut seen,
            &mut candidates,
        );
    }

    let lexical_result = search_lexical_overlay_candidates(
        request.terms,
        &documents,
        request.source_snapshot,
        per_term_limit,
        remaining,
    );
    for hit in lexical_result.candidates {
        if candidates.len() >= request.limit {
            break;
        }
        let candidate = DynamicSearchCandidate {
            path: hit.owner_path().to_string(),
            line: 1,
            end_line: 1,
            symbol: hit.symbol().to_string(),
            text: hit.text().to_string(),
            source: SEARCH_OVERLAY_ROUTE_SOURCE.to_string(),
            confidence: "lexical-overlay".to_string(),
        };
        push_candidate(candidate, &mut seen, &mut candidates);
    }

    DynamicSearchCandidateCollection {
        source_snapshot: lexical_result.source_snapshot,
        candidates,
    }
}

fn resolved_owner_roots(project_root: &Path, owners: &[PathBuf]) -> Vec<PathBuf> {
    if owners.is_empty() {
        return Vec::new();
    }
    owners
        .iter()
        .map(|owner| {
            if owner.is_absolute() {
                owner.clone()
            } else {
                project_root.join(owner)
            }
        })
        .collect()
}

struct CommittedDynamicOwner {
    path: PathBuf,
    source: Vec<u8>,
}

fn sorted_search_root_files(
    root: &Path,
    request: &DynamicSearchRootCandidateRequest<'_>,
) -> Result<Vec<CommittedDynamicOwner>, String> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let metadata = std::fs::metadata(root).map_err(|error| {
        format!(
            "failed to inspect search pipe root {}: {error}",
            root.display()
        )
    })?;
    if metadata.is_file() {
        if !(request.file_matches)(root) {
            return Ok(Vec::new());
        }
        let owner_path = display_path(request.project_root, root);
        let expected = request
            .base_snapshot
            .file_digest(&owner_path)
            .ok_or_else(|| {
                format!("dynamic lexical owner is absent from committed snapshot: {owner_path}")
            })?;
        let source = std::fs::read(root).map_err(|error| {
            format!("failed to read committed search owner {owner_path}: {error}")
        })?;
        let observed =
            agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1(
                &source,
            );
        if observed.as_str() != expected {
            return Err(format!("dynamic lexical owner content drift: {owner_path}"));
        }
        return Ok(vec![CommittedDynamicOwner {
            path: root.to_path_buf(),
            source,
        }]);
    }
    Err(format!(
        "dynamic lexical overlay directory scan is removed; provide explicit Merkle-admitted owner files: {}",
        root.display()
    ))
}

fn append_overlay_path_candidates(
    locator_root: &Path,
    terms: &[String],
    per_term_limit: usize,
    owners: &[CommittedDynamicOwner],
    remaining: &mut usize,
    seen: &mut HashSet<String>,
    candidates: &mut Vec<DynamicSearchCandidate>,
) {
    let mut term_counts = vec![0usize; terms.len()];
    for owner in owners {
        if *remaining == 0 {
            break;
        }
        let display = display_path(locator_root, &owner.path);
        let lower = display.to_ascii_lowercase();
        for (index, term) in terms.iter().enumerate() {
            if term_counts[index] >= per_term_limit || !lower.contains(term) {
                continue;
            }
            let candidate = DynamicSearchCandidate {
                path: display.clone(),
                line: 1,
                end_line: 1,
                symbol: term.clone(),
                text: display.clone(),
                source: SEARCH_OVERLAY_ROUTE_SOURCE.to_string(),
                confidence: "path-lexical-overlay".to_string(),
            };
            if push_candidate(candidate, seen, candidates) {
                term_counts[index] += 1;
                *remaining -= 1;
            }
            break;
        }
    }
}

fn lexical_overlay_document(
    locator_root: &Path,
    owner: &CommittedDynamicOwner,
) -> LexicalOverlayDocument {
    let display = display_path(locator_root, &owner.path);
    let source_hash = blake3::hash(&owner.source).to_hex().to_string();
    let source_text = String::from_utf8_lossy(&owner.source).into_owned();
    LexicalOverlayDocument::new(display.clone(), display.clone(), symbol_from_text(&display))
        .kind("owner")
        .source_hash(source_hash)
        .search_text(source_text)
}

fn owner_line_matches(source: &[u8], line_number: usize, expected: &[u8]) -> bool {
    if line_number == 0 {
        return false;
    }
    source
        .split(|byte| *byte == b'\n')
        .nth(line_number - 1)
        .map(|line| line.strip_suffix(b"\r").unwrap_or(line) == expected)
        .unwrap_or(false)
}

fn rg_coverage_input_digest(request: &RgCoverageRequest<'_>) -> String {
    let mut hasher = blake3::Hasher::new();
    for value in [
        "agent.semantic-protocols.rg-coverage-input.v1",
        request.generation_digest,
        request.source_snapshot.root_digest.as_str(),
        request.source_snapshot.provider_digest.as_str(),
    ] {
        hasher.update(&(value.len() as u64).to_le_bytes());
        hasher.update(value.as_bytes());
    }
    let mut owners = request.owners.iter().collect::<Vec<_>>();
    owners.sort_by_key(|owner| owner.owner_path);
    for owner in owners {
        for value in [owner.owner_path, owner.content_digest] {
            hasher.update(&(value.len() as u64).to_le_bytes());
            hasher.update(value.as_bytes());
        }
    }
    hasher.update(&(request.output.len() as u64).to_le_bytes());
    hasher.update(request.output);
    format!("blake3-256:{}", hasher.finalize().to_hex())
}

fn push_candidate(
    candidate: DynamicSearchCandidate,
    seen: &mut HashSet<String>,
    candidates: &mut Vec<DynamicSearchCandidate>,
) -> bool {
    let key = format!(
        "{}:{}:{}:{}:{}",
        candidate.path, candidate.line, candidate.symbol, candidate.source, candidate.confidence
    );
    if !seen.insert(key) {
        return false;
    }
    candidates.push(candidate);
    true
}

fn per_term_candidate_limit(term_count: usize, total_limit: usize) -> usize {
    if term_count == 0 {
        return total_limit;
    }
    (total_limit / term_count).clamp(16, 64).min(total_limit)
}

fn symbol_from_text(text: &str) -> String {
    text.split(|character: char| {
        !(character == '_' || character == '-' || character.is_ascii_alphanumeric())
    })
    .find(|part| !part.is_empty())
    .unwrap_or("match")
    .to_lowercase()
}

fn display_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn symbol_from_bytes(bytes: &[u8]) -> String {
    symbol_from_text(&byte_text::lossy_string(bytes))
}

fn parse_usize_ascii(bytes: &[u8]) -> Option<usize> {
    std::str::from_utf8(bytes).ok()?.parse::<usize>().ok()
}

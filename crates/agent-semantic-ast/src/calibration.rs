// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Frontier 2: Dual-Track Sound Zero-Round-Trip Syntax Calibration (RFC 10.35).
//!
//! Never duplicates a full language grammar. Parameterized solely by the
//! registered provider's [`LexicalDelimiterProfile`], this module proposes a
//! bounded set (`<= 8`) of pure delimiter/punctuation repairs (`cost <= 2`)
//! anchored to the edited hole or native parser error span, then batch-verifies
//! every candidate against the authoritative native parser oracle
//! (`AstProviderPort::verify`).

use std::collections::HashSet;

use crate::provider_port::{AstProviderPort, AstVerifyCandidate, AstVerifyRequest};
use crate::span_overlay::blake3_source_digest;
use crate::types::{
    AstCalibrationEdit, AstCalibrationEditKind, AstCalibrationOutcome, AstEngineError,
    AstNodeLocator, AstProviderId, AstSyntaxDiagnostic, AstWorkspacePath, Blake3SourceDigest,
    ByteSpan, CalibrationReceipt, LexicalDelimiterProfile, TypedHoleDiagnostic,
};

const MAX_CALIBRATION_CANDIDATES: usize = 8;
const MAX_DELIMITER_REPAIR_COST: u32 = 2;

/// Request invoking Frontier 2 oracle-verified syntax calibration.
pub struct CalibrationRequest<'a> {
    pub path: &'a AstWorkspacePath,
    pub preimage_hash: &'a Blake3SourceDigest,
    pub raw_source: &'a str,
    pub edited_span: ByteSpan,
    pub locator: Option<AstNodeLocator>,
    pub allow_auto_calibrate: bool,
}

/// Outcome returned by [`calibrate_with_oracle`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CalibrationOutcome {
    pub calibrated_source: Option<String>,
    pub receipt: CalibrationReceipt,
    pub typed_hole: Option<TypedHoleDiagnostic>,
}

#[derive(Clone, Debug)]
struct DelimiterScanSummary {
    unclosed_stack: Vec<(char, usize)>,
    unmatched_closers: Vec<(char, usize)>,
}

#[derive(Clone, Debug)]
struct ProposedCandidate {
    source: String,
    edits: Vec<AstCalibrationEdit>,
    total_cost: u32,
}

fn matching_closer(open: char) -> Option<char> {
    match open {
        '(' => Some(')'),
        '{' => Some('}'),
        '[' => Some(']'),
        _ => None,
    }
}

fn is_closer(ch: char) -> bool {
    matches!(ch, ')' | '}' | ']')
}

fn count_leading_hashes(bytes: &[u8], start: usize) -> usize {
    bytes[start..].iter().take_while(|&&b| b == b'#').count()
}

fn matches_closing_hashes(bytes: &[u8], quote_pos: usize, hashes: usize) -> bool {
    (0..hashes).all(|h| bytes.get(quote_pos + 1 + h) == Some(&b'#'))
}

fn try_skip_rust_raw_string(bytes: &[u8], start: usize) -> Option<usize> {
    if bytes.get(start) != Some(&b'r') {
        return None;
    }
    let hashes = count_leading_hashes(bytes, start + 1);
    let open_quote = start + 1 + hashes;
    if bytes.get(open_quote) != Some(&b'"') {
        return None;
    }
    let body_start = open_quote + 1;
    let close_quote = (body_start..bytes.len())
        .find(|&pos| bytes[pos] == b'"' && matches_closing_hashes(bytes, pos, hashes));
    Some(close_quote.map_or(bytes.len(), |pos| pos + 1 + hashes))
}

fn skip_quoted(bytes: &[u8], start: usize, quote: u8) -> usize {
    let mut i = start + 1;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i = (i + 2).min(bytes.len());
            continue;
        }
        if bytes[i] == quote {
            return i + 1;
        }
        i += 1;
    }
    bytes.len()
}

fn skip_line_comment(bytes: &[u8], start: usize, prefix_len: usize) -> usize {
    let after_prefix = start + prefix_len;
    bytes[after_prefix..]
        .iter()
        .position(|&b| b == b'\n')
        .map_or(bytes.len(), |off| after_prefix + off)
}

fn skip_block_comment(bytes: &[u8], start: usize, open_len: usize, close: &[u8]) -> usize {
    let after_open = start + open_len;
    bytes[after_open..]
        .windows(close.len())
        .position(|win| win == close)
        .map_or(bytes.len(), |off| {
            (after_open + off + close.len()).min(bytes.len())
        })
}

fn try_skip_non_delimiter_token(
    bytes: &[u8],
    pos: usize,
    profile: &LexicalDelimiterProfile,
) -> Option<usize> {
    let line_comment = profile.line_comment.as_bytes();
    if !line_comment.is_empty() && bytes[pos..].starts_with(line_comment) {
        return Some(skip_line_comment(bytes, pos, line_comment.len()));
    }
    if let (Some(open), Some(close)) = (
        profile.block_comment_open.as_deref().map(str::as_bytes),
        profile.block_comment_close.as_deref().map(str::as_bytes),
    ) && bytes[pos..].starts_with(open)
    {
        return Some(skip_block_comment(bytes, pos, open.len(), close));
    }
    if profile.supports_rust_raw_strings
        && let Some(next_pos) = try_skip_rust_raw_string(bytes, pos)
    {
        return Some(next_pos);
    }
    if bytes[pos] == b'"' {
        return Some(skip_quoted(bytes, pos, b'"'));
    }
    try_skip_single_quote(bytes, pos, profile.supports_single_quote_strings)
}

fn try_skip_single_quote(
    bytes: &[u8],
    pos: usize,
    supports_single_quote_strings: bool,
) -> Option<usize> {
    if bytes[pos] != b'\'' {
        return None;
    }
    if supports_single_quote_strings {
        return Some(skip_quoted(bytes, pos, b'\''));
    }
    if bytes.get(pos + 1) == Some(&b'\\') && bytes.get(pos + 3) == Some(&b'\'') {
        return Some(pos + 4);
    }
    if bytes.get(pos + 2) == Some(&b'\'') {
        return Some(pos + 3);
    }
    None
}

fn record_delimiter_byte(
    byte: u8,
    pos: usize,
    stack: &mut Vec<(char, usize)>,
    unmatched: &mut Vec<(char, usize)>,
) {
    let ch = byte as char;
    if let Some(closer) = matching_closer(ch) {
        stack.push((closer, pos));
    } else if is_closer(ch) {
        if stack.last().is_some_and(|(expected, _)| *expected == ch) {
            stack.pop();
        } else {
            unmatched.push((ch, pos));
        }
    }
}

fn scan_delimiter_skeleton(
    source: &str,
    profile: &LexicalDelimiterProfile,
) -> DelimiterScanSummary {
    let bytes = source.as_bytes();
    let mut stack = Vec::new();
    let mut unmatched = Vec::new();
    let mut pos = 0usize;
    while pos < bytes.len() {
        if let Some(next_pos) = try_skip_non_delimiter_token(bytes, pos, profile) {
            pos = next_pos;
            continue;
        }
        record_delimiter_byte(bytes[pos], pos, &mut stack, &mut unmatched);
        pos += 1;
    }
    DelimiterScanSummary {
        unclosed_stack: stack,
        unmatched_closers: unmatched,
    }
}

fn push_unique_candidate(
    seen: &mut HashSet<String>,
    out: &mut Vec<ProposedCandidate>,
    candidate: ProposedCandidate,
) {
    if out.len() < MAX_CALIBRATION_CANDIDATES && seen.insert(candidate.source.clone()) {
        out.push(candidate);
    }
}

fn next_non_ws_offset(source: &str, offset: usize) -> usize {
    let clamped = offset.min(source.len());
    let ws_bytes = source.as_bytes()[clamped..]
        .iter()
        .take_while(|b| b.is_ascii_whitespace())
        .count();
    clamped + ws_bytes
}

fn dedup_whitespace_equivalent_sites(source: &str, raw_sites: Vec<usize>) -> Vec<usize> {
    let mut seen_anchors = HashSet::new();
    raw_sites
        .into_iter()
        .map(|site| site.min(source.len()))
        .filter(|&site| seen_anchors.insert(next_non_ws_offset(source, site)))
        .collect()
}

fn insert_at_offset(source: &str, offset: usize, text: &str) -> Option<String> {
    let clamped = offset.min(source.len());
    if !source.is_char_boundary(clamped) {
        return None;
    }
    let mut out = String::with_capacity(source.len() + text.len());
    out.push_str(&source[..clamped]);
    out.push_str(text);
    out.push_str(&source[clamped..]);
    Some(out)
}

fn delete_offsets(source: &str, offsets: &[usize]) -> Option<String> {
    let mut sorted = offsets.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    let mut out = String::with_capacity(source.len());
    let mut cursor = 0usize;
    for pos in sorted {
        if pos >= source.len() || !source.is_char_boundary(pos) || !source.is_char_boundary(pos + 1)
        {
            return None;
        }
        out.push_str(&source[cursor..pos]);
        cursor = pos + 1;
    }
    out.push_str(&source[cursor..]);
    Some(out)
}

fn is_markdown_fence_line(line: &&str) -> bool {
    line.trim_start().starts_with("```")
}

fn strip_markdown_fences_in_source(source: &str) -> Option<ProposedCandidate> {
    if !source.lines().any(|line| is_markdown_fence_line(&line)) {
        return None;
    }
    let kept: Vec<&str> = source
        .lines()
        .filter(|line| !is_markdown_fence_line(line))
        .collect();
    let mut rebuilt = kept.join("\n");
    if source.ends_with('\n') {
        rebuilt.push('\n');
    }
    Some(ProposedCandidate {
        source: rebuilt,
        edits: vec![AstCalibrationEdit {
            kind: AstCalibrationEditKind::StripCodeFence,
            byte_offset: 0,
            text: "```".to_owned(),
            cost: 1,
        }],
        total_cost: 1,
    })
}

fn propose_candidates(
    source: &str,
    profile: &LexicalDelimiterProfile,
    edited_span: ByteSpan,
    diagnostics: &[AstSyntaxDiagnostic],
) -> Vec<ProposedCandidate> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    if let Some(stripped) = strip_markdown_fences_in_source(source) {
        push_unique_candidate(&mut seen, &mut out, stripped);
    }
    let summary = scan_delimiter_skeleton(source, profile);
    let diag_offset = diagnostics.first().map(|d| d.byte_start.min(source.len()));
    propose_missing_closers(
        source,
        &summary,
        edited_span,
        diag_offset,
        &mut seen,
        &mut out,
    );
    propose_extra_closer_deletions(source, &summary, &mut seen, &mut out);
    propose_missing_terminator(
        source,
        profile,
        diag_offset,
        diagnostics,
        &mut seen,
        &mut out,
    );
    out
}

fn propose_missing_closers(
    source: &str,
    summary: &DelimiterScanSummary,
    edited_span: ByteSpan,
    diag_offset: Option<usize>,
    seen: &mut HashSet<String>,
    out: &mut Vec<ProposedCandidate>,
) {
    if summary.unclosed_stack.is_empty()
        || summary.unclosed_stack.len() > MAX_DELIMITER_REPAIR_COST as usize
    {
        return;
    }
    let closers: String = summary
        .unclosed_stack
        .iter()
        .rev()
        .map(|(ch, _)| *ch)
        .collect();
    let cost = u32::try_from(closers.len()).unwrap_or(MAX_DELIMITER_REPAIR_COST);
    let mut raw_sites = vec![edited_span.end.min(source.len())];
    if let Some(off) = diag_offset {
        raw_sites.push(off);
    }
    raw_sites.push(source.len());
    for site in dedup_whitespace_equivalent_sites(source, raw_sites) {
        if let Some(fixed) = insert_at_offset(source, site, &closers) {
            push_unique_candidate(
                seen,
                out,
                ProposedCandidate {
                    source: fixed,
                    edits: vec![AstCalibrationEdit {
                        kind: AstCalibrationEditKind::InsertDelimiter,
                        byte_offset: site,
                        text: closers.clone(),
                        cost,
                    }],
                    total_cost: cost,
                },
            );
        }
    }
}

fn propose_extra_closer_deletions(
    source: &str,
    summary: &DelimiterScanSummary,
    seen: &mut HashSet<String>,
    out: &mut Vec<ProposedCandidate>,
) {
    if summary.unmatched_closers.is_empty()
        || summary.unmatched_closers.len() > MAX_DELIMITER_REPAIR_COST as usize
    {
        return;
    }
    let offsets: Vec<usize> = summary.unmatched_closers.iter().map(|(_, p)| *p).collect();
    if let Some(fixed) = delete_offsets(source, &offsets) {
        let edits: Vec<AstCalibrationEdit> = summary
            .unmatched_closers
            .iter()
            .map(|(ch, pos)| AstCalibrationEdit {
                kind: AstCalibrationEditKind::DeleteDelimiter,
                byte_offset: *pos,
                text: ch.to_string(),
                cost: 1,
            })
            .collect();
        let cost = u32::try_from(edits.len()).unwrap_or(MAX_DELIMITER_REPAIR_COST);
        push_unique_candidate(
            seen,
            out,
            ProposedCandidate {
                source: fixed,
                edits,
                total_cost: cost,
            },
        );
    }
}

fn propose_missing_terminator(
    source: &str,
    profile: &LexicalDelimiterProfile,
    diag_offset: Option<usize>,
    diagnostics: &[AstSyntaxDiagnostic],
    seen: &mut HashSet<String>,
    out: &mut Vec<ProposedCandidate>,
) {
    let Some(term) = profile.statement_terminator else {
        return;
    };
    let expects_term = diagnostics.iter().any(|d| {
        let msg = d.message.to_ascii_lowercase();
        msg.contains("expected `;`")
            || msg.contains("semicolon")
            || d.expected_tokens.iter().any(|tok| tok == ";")
    });
    let Some(off) = diag_offset.filter(|_| expects_term) else {
        return;
    };
    let mut sites = vec![off];
    if let Some(prev_nl) = source[..off].rfind('\n') {
        sites.push(prev_nl);
    }
    for site in dedup_whitespace_equivalent_sites(source, sites) {
        if let Some(fixed) = insert_at_offset(source, site, &term.to_string()) {
            push_unique_candidate(
                seen,
                out,
                ProposedCandidate {
                    source: fixed,
                    edits: vec![AstCalibrationEdit {
                        kind: AstCalibrationEditKind::InsertTerminator,
                        byte_offset: site,
                        text: term.to_string(),
                        cost: 1,
                    }],
                    total_cost: 1,
                },
            );
        }
    }
}

fn build_rejected_outcome(
    request: &CalibrationRequest<'_>,
    raw_hash: Blake3SourceDigest,
    provider_id: &AstProviderId,
    outcome: AstCalibrationOutcome,
    reason: String,
    diagnostics: &[AstSyntaxDiagnostic],
    ambiguous_candidates: usize,
) -> CalibrationOutcome {
    let span = diagnostics
        .first()
        .and_then(|d| ByteSpan::new(d.byte_start, d.byte_end.max(d.byte_start)).ok())
        .unwrap_or(request.edited_span);
    let expected = diagnostics
        .iter()
        .flat_map(|d| d.expected_tokens.iter().cloned())
        .collect();
    CalibrationOutcome {
        calibrated_source: None,
        receipt: CalibrationReceipt {
            preimage_hash: request.preimage_hash.clone(),
            raw_hash,
            calibrated_hash: None,
            outcome,
            total_cost: 0,
            edits: Vec::new(),
            oracle_provider_id: provider_id.clone(),
        },
        typed_hole: Some(TypedHoleDiagnostic {
            locator: request.locator.clone(),
            span,
            reason,
            expected,
            ambiguous_candidates,
        }),
    }
}

/// Run Frontier 2 Oracle-Verified Bounded Syntax Calibration against `provider`.
pub fn calibrate_with_oracle(
    provider: &dyn AstProviderPort,
    request: &CalibrationRequest<'_>,
) -> Result<CalibrationOutcome, AstEngineError> {
    let raw_hash = blake3_source_digest(request.raw_source.as_bytes());
    let fast_resp = provider.verify(&AstVerifyRequest {
        path: request.path.clone(),
        candidates: vec![AstVerifyCandidate {
            candidate_id: "raw".to_owned(),
            source_utf8: request.raw_source.to_owned(),
        }],
    })?;
    let first = fast_resp.results.first().ok_or_else(|| {
        AstEngineError::ProviderError("ast-verify returned empty results".to_owned())
    })?;
    if first.accepted {
        return Ok(exact_fast_path_outcome(
            request,
            raw_hash,
            provider.provider_id(),
        ));
    }
    if !request.allow_auto_calibrate {
        return Ok(reject_with_first_diagnostic(
            request,
            raw_hash,
            provider.provider_id(),
            &first.diagnostics,
            "native syntax verification failed",
        ));
    }
    run_bounded_calibration_search(provider, request, raw_hash, &first.diagnostics)
}

fn exact_fast_path_outcome(
    request: &CalibrationRequest<'_>,
    raw_hash: Blake3SourceDigest,
    provider_id: &AstProviderId,
) -> CalibrationOutcome {
    CalibrationOutcome {
        calibrated_source: Some(request.raw_source.to_owned()),
        receipt: CalibrationReceipt {
            preimage_hash: request.preimage_hash.clone(),
            raw_hash: raw_hash.clone(),
            calibrated_hash: Some(raw_hash),
            outcome: AstCalibrationOutcome::ExactFastPath,
            total_cost: 0,
            edits: Vec::new(),
            oracle_provider_id: provider_id.clone(),
        },
        typed_hole: None,
    }
}

fn reject_with_first_diagnostic(
    request: &CalibrationRequest<'_>,
    raw_hash: Blake3SourceDigest,
    provider_id: &AstProviderId,
    diagnostics: &[AstSyntaxDiagnostic],
    fallback: &str,
) -> CalibrationOutcome {
    let reason = diagnostics
        .first()
        .map_or_else(|| fallback.to_owned(), |d| d.message.clone());
    build_rejected_outcome(
        request,
        raw_hash,
        provider_id,
        AstCalibrationOutcome::RejectedTypedHole,
        reason,
        diagnostics,
        0,
    )
}

fn run_bounded_calibration_search(
    provider: &dyn AstProviderPort,
    request: &CalibrationRequest<'_>,
    raw_hash: Blake3SourceDigest,
    diagnostics: &[AstSyntaxDiagnostic],
) -> Result<CalibrationOutcome, AstEngineError> {
    let profile = provider.lexical_profile();
    let proposals = propose_candidates(
        request.raw_source,
        &profile,
        request.edited_span,
        diagnostics,
    );
    if proposals.is_empty() {
        return Ok(reject_with_first_diagnostic(
            request,
            raw_hash,
            provider.provider_id(),
            diagnostics,
            "syntax error outside bounded delimiter calibration",
        ));
    }
    let verify_candidates: Vec<AstVerifyCandidate> = proposals
        .iter()
        .enumerate()
        .map(|(idx, cand)| AstVerifyCandidate {
            candidate_id: format!("cand-{idx}"),
            source_utf8: cand.source.clone(),
        })
        .collect();
    let batch_resp = provider.verify(&AstVerifyRequest {
        path: request.path.clone(),
        candidates: verify_candidates,
    })?;
    let passing: Vec<&ProposedCandidate> = batch_resp
        .results
        .iter()
        .enumerate()
        .filter_map(|(idx, res)| res.accepted.then(|| proposals.get(idx)).flatten())
        .collect();
    let Some(min_cost) = passing.iter().map(|c| c.total_cost).min() else {
        return Ok(reject_with_first_diagnostic(
            request,
            raw_hash,
            provider.provider_id(),
            diagnostics,
            "no bounded calibration candidate passed native oracle",
        ));
    };
    let minimal: Vec<&ProposedCandidate> = passing
        .into_iter()
        .filter(|c| c.total_cost == min_cost)
        .collect();
    if minimal.len() > 1 {
        return Ok(build_rejected_outcome(
            request,
            raw_hash,
            provider.provider_id(),
            AstCalibrationOutcome::RejectedAmbiguousTie,
            format!(
                "ambiguous calibration tie: {} distinct minimum-cost (cost={min_cost}) repairs passed oracle; refusing to guess",
                minimal.len()
            ),
            diagnostics,
            minimal.len(),
        ));
    }
    let winner = minimal[0];
    let calibrated_hash = blake3_source_digest(winner.source.as_bytes());
    Ok(CalibrationOutcome {
        calibrated_source: Some(winner.source.clone()),
        receipt: CalibrationReceipt {
            preimage_hash: request.preimage_hash.clone(),
            raw_hash,
            calibrated_hash: Some(calibrated_hash),
            outcome: AstCalibrationOutcome::CalibratedUniqueMinCost,
            total_cost: winner.total_cost,
            edits: winner.edits.clone(),
            oracle_provider_id: provider.provider_id().clone(),
        },
        typed_hole: None,
    })
}

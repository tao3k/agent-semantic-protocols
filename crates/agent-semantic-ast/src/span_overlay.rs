// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Source-Backed Span-Indexed Lossless Overlay (RFC 10.35).
//!
//! Computes structural byte-range replacements directly against immutable
//! BLAKE3-hashed UTF-8 source bytes so every untouched comment, doc-comment,
//! blank line, and formatting byte stays bit-for-bit identical — even when the
//! authoritative parser is AST-only (such as Rust `syn` or Python `ast`).

use std::collections::HashMap;
use std::fmt::Write as _;

use crate::types::{
    AstEngineError, AstNodeLocator, AstNodeSpanRecord, AstPatchOp, AstUniversalKind,
    Blake3SourceDigest, ByteSpan,
};

/// One UTF-8 byte-range replacement anchored into an immutable source buffer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpanReplacement {
    pub span: ByteSpan,
    pub replacement: String,
}

/// Compute canonical `blake3-256:<hex>` digest over immutable UTF-8 source bytes.
#[must_use]
pub fn blake3_source_digest(bytes: &[u8]) -> Blake3SourceDigest {
    let hex = blake3::hash(bytes).to_hex();
    Blake3SourceDigest::new(format!("blake3-256:{hex}")).expect("blake3-256 digest is always valid")
}

/// Splice non-overlapping byte-range replacements into `source`, guaranteeing
/// bit-for-bit preservation of every untouched byte outside edited spans.
pub fn splice_lossless(source: &str, edits: &[SpanReplacement]) -> Result<String, AstEngineError> {
    if edits.is_empty() {
        return Ok(source.to_owned());
    }
    let mut sorted = edits.to_vec();
    sorted.sort_by_key(|e| (e.span.start, e.span.end));
    validate_disjoint_spans(source, &sorted)?;
    Ok(apply_sorted_replacements(source, &sorted))
}

fn validate_disjoint_spans(source: &str, sorted: &[SpanReplacement]) -> Result<(), AstEngineError> {
    let mut prev_end = 0usize;
    for edit in sorted {
        if edit.span.end > source.len()
            || !source.is_char_boundary(edit.span.start)
            || !source.is_char_boundary(edit.span.end)
        {
            return Err(AstEngineError::InvalidSpan(
                "edit span out of bounds or not on UTF-8 boundary",
            ));
        }
        if edit.span.start < prev_end {
            return Err(AstEngineError::InvalidSpan(
                "overlapping AST span replacements rejected fail-closed",
            ));
        }
        prev_end = edit.span.end;
    }
    Ok(())
}

fn apply_sorted_replacements(source: &str, sorted: &[SpanReplacement]) -> String {
    let mut out = String::with_capacity(source.len());
    let mut cursor = 0usize;
    for edit in sorted {
        out.push_str(&source[cursor..edit.span.start]);
        out.push_str(&edit.replacement);
        cursor = edit.span.end;
    }
    out.push_str(&source[cursor..]);
    out
}

fn line_indent_at(source: &str, offset: usize) -> &str {
    let prefix = &source[..offset.min(source.len())];
    let line_start = prefix.rfind('\n').map_or(0, |idx| idx + 1);
    let line = &source[line_start..offset.min(source.len())];
    let ws_len = line
        .bytes()
        .take_while(|b| *b == b' ' || *b == b'\t')
        .count();
    &line[..ws_len]
}

fn indent_snippet_lines(snippet: &str, indent: &str) -> String {
    let trimmed = snippet.trim_end_matches('\n');
    let mut out = String::new();
    for (idx, line) in trimmed.lines().enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        if !line.trim().is_empty() {
            out.push_str(indent);
            out.push_str(line);
        }
    }
    out
}

fn lookup_node<'a>(
    node_map: &HashMap<&'a str, &'a AstNodeSpanRecord>,
    locator: &AstNodeLocator,
) -> Result<&'a AstNodeSpanRecord, AstEngineError> {
    node_map
        .get(locator.as_str())
        .copied()
        .ok_or_else(|| AstEngineError::LocatorNotFound(locator.as_str().to_owned()))
}

/// Resolve a high-level [`AstPatchOp`] against the native AST node span table
/// into exact non-overlapping [`SpanReplacement`]s over `source`.
pub fn compute_op_span_edits(
    source: &str,
    nodes: &[AstNodeSpanRecord],
    op: &AstPatchOp,
) -> Result<Vec<SpanReplacement>, AstEngineError> {
    let node_map: HashMap<&str, &AstNodeSpanRecord> = nodes
        .iter()
        .map(|node| (node.locator.as_str(), node))
        .collect();
    match op {
        AstPatchOp::ReplaceItem { target, snippet } => {
            edit_replace_item(&node_map, target, snippet)
        }
        AstPatchOp::AppendToBlock { target, snippet } => {
            edit_append_to_block(source, &node_map, target, snippet)
        }
        AstPatchOp::InsertBeforeNode { target, snippet } => {
            edit_insert_before(source, &node_map, target, snippet)
        }
        AstPatchOp::InsertAfterNode { target, snippet } => {
            edit_insert_after(source, &node_map, target, snippet)
        }
        AstPatchOp::InsertImport { import_statement } => {
            edit_insert_import(source, nodes, import_statement)
        }
        AstPatchOp::RemoveNode { target } => edit_remove_node(source, &node_map, target),
        AstPatchOp::ApplySpanEdits { edits } => Ok(edits
            .iter()
            .map(|(span, replacement)| SpanReplacement {
                span: *span,
                replacement: replacement.clone(),
            })
            .collect()),
    }
}

fn edit_replace_item(
    node_map: &HashMap<&str, &AstNodeSpanRecord>,
    target: &AstNodeLocator,
    snippet: &str,
) -> Result<Vec<SpanReplacement>, AstEngineError> {
    let node = lookup_node(node_map, target)?;
    let replace_outer =
        snippet.trim_start().starts_with("///") || snippet.trim_start().starts_with("#[");
    let span = if replace_outer {
        node.outer_span
    } else {
        node.inner_span
    };
    Ok(vec![SpanReplacement {
        span,
        replacement: snippet.trim_end_matches('\n').to_owned(),
    }])
}

fn edit_append_to_block(
    source: &str,
    node_map: &HashMap<&str, &AstNodeSpanRecord>,
    target: &AstNodeLocator,
    snippet: &str,
) -> Result<Vec<SpanReplacement>, AstEngineError> {
    let node = lookup_node(node_map, target)?;
    let body = node.body_span.ok_or(AstEngineError::InvalidSpan(
        "target AST node does not expose a block body span",
    ))?;
    let outer_indent = line_indent_at(source, node.inner_span.start);
    let inner_indent = format!("{outer_indent}    ");
    let formatted = indent_snippet_lines(snippet, &inner_indent);
    let needs_leading_nl = !source[body.start..body.end].ends_with('\n');
    let leading = if needs_leading_nl { "\n" } else { "" };
    Ok(vec![SpanReplacement {
        span: ByteSpan::new(body.end, body.end)?,
        replacement: format!("{leading}{formatted}\n{outer_indent}"),
    }])
}

fn edit_insert_before(
    source: &str,
    node_map: &HashMap<&str, &AstNodeSpanRecord>,
    target: &AstNodeLocator,
    snippet: &str,
) -> Result<Vec<SpanReplacement>, AstEngineError> {
    let node = lookup_node(node_map, target)?;
    let indent = line_indent_at(source, node.outer_span.start);
    let formatted = indent_snippet_lines(snippet, indent);
    let pos = node.outer_span.start;
    Ok(vec![SpanReplacement {
        span: ByteSpan::new(pos, pos)?,
        replacement: format!("{formatted}\n{indent}"),
    }])
}

fn edit_insert_after(
    source: &str,
    node_map: &HashMap<&str, &AstNodeSpanRecord>,
    target: &AstNodeLocator,
    snippet: &str,
) -> Result<Vec<SpanReplacement>, AstEngineError> {
    let node = lookup_node(node_map, target)?;
    let indent = line_indent_at(source, node.outer_span.start);
    let formatted = indent_snippet_lines(snippet, indent);
    let pos = node.outer_span.end;
    Ok(vec![SpanReplacement {
        span: ByteSpan::new(pos, pos)?,
        replacement: format!("\n{formatted}"),
    }])
}

fn edit_insert_import(
    source: &str,
    nodes: &[AstNodeSpanRecord],
    import_statement: &str,
) -> Result<Vec<SpanReplacement>, AstEngineError> {
    let clean = import_statement.trim();
    if source.lines().any(|line| line.trim() == clean) {
        return Ok(Vec::new());
    }
    let last_import = nodes
        .iter()
        .filter(|node| node.kind == AstUniversalKind::Import)
        .max_by_key(|node| node.outer_span.end);
    match last_import {
        Some(node) => Ok(vec![SpanReplacement {
            span: ByteSpan::new(node.outer_span.end, node.outer_span.end)?,
            replacement: format!("\n{clean}"),
        }]),
        None => Ok(vec![SpanReplacement {
            span: ByteSpan::new(0, 0)?,
            replacement: format!("{clean}\n"),
        }]),
    }
}

fn edit_remove_node(
    source: &str,
    node_map: &HashMap<&str, &AstNodeSpanRecord>,
    target: &AstNodeLocator,
) -> Result<Vec<SpanReplacement>, AstEngineError> {
    let node = lookup_node(node_map, target)?;
    let end = if source.as_bytes().get(node.outer_span.end) == Some(&b'\n') {
        node.outer_span.end + 1
    } else {
        node.outer_span.end
    };
    Ok(vec![SpanReplacement {
        span: ByteSpan::new(node.outer_span.start, end)?,
        replacement: String::new(),
    }])
}

/// Render a concise unified audit diff for human/agent inspection (never mutation input).
#[must_use]
pub fn render_compact_unified_diff(path: &str, before: &str, after: &str) -> String {
    if before == after {
        return String::new();
    }
    let old_lines: Vec<&str> = before.lines().collect();
    let new_lines: Vec<&str> = after.lines().collect();
    let prefix = common_prefix_len(&old_lines, &new_lines);
    let suffix = common_suffix_len(&old_lines[prefix..], &new_lines[prefix..]);
    format_diff_hunk(
        path,
        prefix,
        &old_lines[prefix..old_lines.len() - suffix],
        &new_lines[prefix..new_lines.len() - suffix],
    )
}

fn common_prefix_len(a: &[&str], b: &[&str]) -> usize {
    a.iter().zip(b.iter()).take_while(|(x, y)| x == y).count()
}

fn common_suffix_len(a: &[&str], b: &[&str]) -> usize {
    a.iter()
        .rev()
        .zip(b.iter().rev())
        .take_while(|(x, y)| x == y)
        .count()
}

fn format_diff_hunk(path: &str, prefix: usize, removed: &[&str], added: &[&str]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "--- a/{path}\n+++ b/{path}");
    let _ = writeln!(
        out,
        "@@ -{},{} +{},{} @@",
        prefix + 1,
        removed.len(),
        prefix + 1,
        added.len()
    );
    for line in removed {
        let _ = writeln!(out, "-{line}");
    }
    for line in added {
        let _ = writeln!(out, "+{line}");
    }
    out
}

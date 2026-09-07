// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Immutable, content-addressed corpus for the single-process cold `rg` lane.

use serde::Deserialize;
use serde::Serialize;

pub const COLD_RG_CORPUS_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.cold-rg-corpus-receipt";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ColdRgCorpusOwner<'a> {
    pub owner_path: &'a str,
    pub content_digest: &'a str,
    pub bytes: &'a [u8],
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ColdRgOwnerSpan {
    pub owner_path: String,
    pub content_digest: String,
    pub start_line: u64,
    pub end_line: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ColdRgCorpusReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub content_generation_digest: String,
    pub owner_count: usize,
    pub corpus_digest: String,
    pub owner_spans_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ColdRgCorpusArtifact {
    pub receipt: ColdRgCorpusReceipt,
    pub bytes: Vec<u8>,
    pub owner_spans: Vec<ColdRgOwnerSpan>,
}

pub fn build_cold_rg_corpus<'a>(
    content_generation_digest: &str,
    owners: impl IntoIterator<Item = ColdRgCorpusOwner<'a>>,
) -> Result<ColdRgCorpusArtifact, String> {
    let content_generation_digest = crate::canonical_blake3_digest(content_generation_digest)?;
    let mut owners = owners.into_iter().collect::<Vec<_>>();
    owners.sort_by(|left, right| left.owner_path.cmp(right.owner_path));
    if owners.is_empty()
        || owners
            .windows(2)
            .any(|pair| pair[0].owner_path == pair[1].owner_path)
    {
        return Err("cold rg corpus owner set is empty or non-canonical".to_owned());
    }
    let mut bytes = Vec::new();
    let mut owner_spans = Vec::with_capacity(owners.len());
    let mut next_line = 1_u64;
    for owner in owners {
        validate_owner_path(owner.owner_path)?;
        let content_digest = crate::canonical_blake3_digest(owner.content_digest)?;
        let actual = format!("blake3-256:{}", blake3::hash(owner.bytes).to_hex());
        if content_digest != actual {
            return Err(format!(
                "cold rg corpus content digest drift: {}",
                owner.owner_path
            ));
        }
        let start_line = next_line;
        bytes.extend_from_slice(owner.bytes);
        let newline_count = owner.bytes.iter().filter(|byte| **byte == b'\n').count() as u64;
        let missing_terminal_newline = !owner.bytes.ends_with(b"\n");
        if missing_terminal_newline {
            bytes.push(b'\n');
        }
        let line_count = (newline_count + u64::from(missing_terminal_newline)).max(1);
        let end_line = start_line + line_count - 1;
        owner_spans.push(ColdRgOwnerSpan {
            owner_path: owner.owner_path.to_owned(),
            content_digest,
            start_line,
            end_line,
        });
        next_line = end_line + 1;
    }
    let corpus_digest = format!("blake3-256:{}", blake3::hash(&bytes).to_hex());
    let spans = serde_json::to_vec(&owner_spans)
        .map_err(|error| format!("encode cold rg owner spans: {error}"))?;
    let owner_spans_digest = format!("blake3-256:{}", blake3::hash(&spans).to_hex());
    Ok(ColdRgCorpusArtifact {
        receipt: ColdRgCorpusReceipt {
            schema_id: COLD_RG_CORPUS_RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            content_generation_digest,
            owner_count: owner_spans.len(),
            corpus_digest,
            owner_spans_digest,
        },
        bytes,
        owner_spans,
    })
}

pub fn owner_for_corpus_line<'a>(
    spans: &'a [ColdRgOwnerSpan],
    line: u64,
) -> Option<&'a ColdRgOwnerSpan> {
    spans
        .binary_search_by(|span| {
            if line < span.start_line {
                std::cmp::Ordering::Greater
            } else if line > span.end_line {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .ok()
        .map(|index| &spans[index])
}

fn validate_owner_path(owner_path: &str) -> Result<(), String> {
    let path = std::path::Path::new(owner_path);
    if owner_path.is_empty()
        || owner_path.bytes().any(|byte| byte.is_ascii_control())
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err("cold rg corpus owner path is not normalized and relative".to_owned());
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/cold_rg_corpus.rs"]
mod tests;

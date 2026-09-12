// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Immutable, content-addressed corpus for the in-process resident GREP engine.

use serde::Deserialize;
use serde::Serialize;
use std::ops::Range;
use std::sync::Arc;

pub const RESIDENT_GREP_CORPUS_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.cold-rg-corpus-receipt";

// The V1 wire identifier above is immutable. Rust modules and types use the
// implementation-neutral resident GREP namespace.

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResidentGrepCorpusOwner<'a> {
    pub owner_path: &'a str,
    pub content_digest: &'a str,
    pub bytes: &'a [u8],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResidentGrepMappedCorpusOwner {
    pub owner_path: String,
    pub content_digest: String,
    pub byte_range: Range<usize>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResidentGrepOwnerSpan {
    pub owner_path: String,
    pub content_digest: String,
    pub start_line: u64,
    pub end_line: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResidentGrepCorpusReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub content_generation_digest: String,
    pub owner_count: usize,
    pub corpus_digest: String,
    pub owner_spans_digest: String,
}

#[derive(Debug)]
pub struct ResidentGrepCorpusArtifact {
    pub receipt: ResidentGrepCorpusReceipt,
    pub owner_spans: Vec<ResidentGrepOwnerSpan>,
    storage: ResidentGrepCorpusStorage,
    owner_byte_ranges: Vec<Range<usize>>,
    corpus_byte_count: usize,
}

#[derive(Debug)]
enum ResidentGrepCorpusStorage {
    Owned(Arc<[u8]>),
    Mapped(Arc<memmap2::Mmap>),
}

impl ResidentGrepCorpusStorage {
    fn bytes(&self) -> &[u8] {
        match self {
            Self::Owned(bytes) => bytes,
            Self::Mapped(mapping) => mapping.as_ref(),
        }
    }
}

impl ResidentGrepCorpusArtifact {
    /// Return one immutable owner directly from the resident corpus.
    ///
    /// Byte ranges are built with the generation and avoid reconstructing a
    /// temporary filesystem tree on the Search request plane.
    #[must_use]
    pub fn owner_bytes(&self, owner_path: &str) -> Option<&[u8]> {
        let index = self
            .owner_spans
            .binary_search_by(|span| span.owner_path.as_str().cmp(owner_path))
            .ok()?;
        self.owner_byte_ranges
            .get(index)
            .map(|range| &self.storage.bytes()[range.clone()])
    }

    pub fn owners(&self) -> impl Iterator<Item = (&ResidentGrepOwnerSpan, &[u8])> {
        self.owner_spans
            .iter()
            .zip(self.owner_byte_ranges.iter())
            .map(|(span, range)| (span, &self.storage.bytes()[range.clone()]))
    }

    #[must_use]
    pub fn corpus_byte_count(&self) -> usize {
        self.corpus_byte_count
    }

    #[must_use]
    pub fn corpus_heap_bytes(&self) -> usize {
        match &self.storage {
            ResidentGrepCorpusStorage::Owned(bytes) => bytes.len(),
            ResidentGrepCorpusStorage::Mapped(_) => 0,
        }
    }
}

pub fn build_resident_grep_corpus<'a>(
    content_generation_digest: &str,
    owners: impl IntoIterator<Item = ResidentGrepCorpusOwner<'a>>,
) -> Result<ResidentGrepCorpusArtifact, String> {
    let content_generation_digest = crate::canonical_blake3_digest(content_generation_digest)?;
    let mut owners = owners.into_iter().collect::<Vec<_>>();
    owners.sort_by(|left, right| left.owner_path.cmp(right.owner_path));
    if owners.is_empty()
        || owners
            .windows(2)
            .any(|pair| pair[0].owner_path == pair[1].owner_path)
    {
        return Err("resident GREP corpus owner set is empty or non-canonical".to_owned());
    }
    let mut bytes = Vec::new();
    let mut owner_spans = Vec::with_capacity(owners.len());
    let mut owner_byte_ranges = Vec::with_capacity(owners.len());
    let mut next_line = 1_u64;
    for owner in owners {
        validate_owner_path(owner.owner_path)?;
        let content_digest = crate::canonical_blake3_digest(owner.content_digest)?;
        let actual = format!("blake3-256:{}", blake3::hash(owner.bytes).to_hex());
        if content_digest != actual {
            return Err(format!(
                "resident GREP corpus content digest drift: {}",
                owner.owner_path
            ));
        }
        let start_line = next_line;
        let byte_start = bytes.len();
        bytes.extend_from_slice(owner.bytes);
        let byte_end = bytes.len();
        let newline_count = owner.bytes.iter().filter(|byte| **byte == b'\n').count() as u64;
        let missing_terminal_newline = !owner.bytes.ends_with(b"\n");
        if missing_terminal_newline {
            bytes.push(b'\n');
        }
        let line_count = (newline_count + u64::from(missing_terminal_newline)).max(1);
        let end_line = start_line + line_count - 1;
        owner_spans.push(ResidentGrepOwnerSpan {
            owner_path: owner.owner_path.to_owned(),
            content_digest,
            start_line,
            end_line,
        });
        owner_byte_ranges.push(byte_start..byte_end);
        next_line = end_line + 1;
    }
    let corpus_digest = format!("blake3-256:{}", blake3::hash(&bytes).to_hex());
    let spans = serde_json::to_vec(&owner_spans)
        .map_err(|error| format!("encode resident GREP owner spans: {error}"))?;
    let owner_spans_digest = format!("blake3-256:{}", blake3::hash(&spans).to_hex());
    let corpus_byte_count = bytes.len();
    Ok(ResidentGrepCorpusArtifact {
        receipt: ResidentGrepCorpusReceipt {
            schema_id: RESIDENT_GREP_CORPUS_RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            content_generation_digest,
            owner_count: owner_spans.len(),
            corpus_digest,
            owner_spans_digest,
        },
        owner_spans,
        storage: ResidentGrepCorpusStorage::Owned(Arc::from(bytes)),
        owner_byte_ranges,
        corpus_byte_count,
    })
}

pub fn open_mapped_resident_grep_corpus(
    content_generation_digest: &str,
    mapping: Arc<memmap2::Mmap>,
    owners: impl IntoIterator<Item = ResidentGrepMappedCorpusOwner>,
) -> Result<ResidentGrepCorpusArtifact, String> {
    let content_generation_digest = crate::canonical_blake3_digest(content_generation_digest)?;
    let mut owners = owners.into_iter().collect::<Vec<_>>();
    owners.sort_by(|left, right| left.owner_path.cmp(&right.owner_path));
    if owners.is_empty()
        || owners
            .windows(2)
            .any(|pair| pair[0].owner_path == pair[1].owner_path)
    {
        return Err("resident GREP corpus owner set is empty or non-canonical".to_owned());
    }
    let mut owner_spans = Vec::with_capacity(owners.len());
    let mut owner_byte_ranges = Vec::with_capacity(owners.len());
    let mut corpus_hasher = blake3::Hasher::new();
    let mut corpus_byte_count = 0usize;
    let mut next_line = 1u64;
    for owner in owners {
        validate_owner_path(&owner.owner_path)?;
        let bytes = mapping
            .get(owner.byte_range.clone())
            .ok_or_else(|| "resident GREP corpus owner exceeds mapped generation".to_owned())?;
        let content_digest = crate::canonical_blake3_digest(&owner.content_digest)?;
        let actual = format!("blake3-256:{}", blake3::hash(bytes).to_hex());
        if content_digest != actual {
            return Err(format!(
                "resident GREP corpus content digest drift: {}",
                owner.owner_path
            ));
        }
        corpus_hasher.update(bytes);
        corpus_byte_count = corpus_byte_count
            .checked_add(bytes.len())
            .ok_or_else(|| "resident GREP corpus byte count overflows".to_owned())?;
        let newline_count = bytes.iter().filter(|byte| **byte == b'\n').count() as u64;
        let missing_terminal_newline = !bytes.ends_with(b"\n");
        if missing_terminal_newline {
            corpus_hasher.update(b"\n");
            corpus_byte_count = corpus_byte_count
                .checked_add(1)
                .ok_or_else(|| "resident GREP corpus byte count overflows".to_owned())?;
        }
        let line_count = (newline_count + u64::from(missing_terminal_newline)).max(1);
        let end_line = next_line + line_count - 1;
        owner_spans.push(ResidentGrepOwnerSpan {
            owner_path: owner.owner_path,
            content_digest,
            start_line: next_line,
            end_line,
        });
        owner_byte_ranges.push(owner.byte_range);
        next_line = end_line + 1;
    }
    let corpus_digest = format!("blake3-256:{}", corpus_hasher.finalize().to_hex());
    let spans = serde_json::to_vec(&owner_spans)
        .map_err(|error| format!("encode resident GREP owner spans: {error}"))?;
    let owner_spans_digest = format!("blake3-256:{}", blake3::hash(&spans).to_hex());
    Ok(ResidentGrepCorpusArtifact {
        receipt: ResidentGrepCorpusReceipt {
            schema_id: RESIDENT_GREP_CORPUS_RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            content_generation_digest,
            owner_count: owner_spans.len(),
            corpus_digest,
            owner_spans_digest,
        },
        owner_spans,
        storage: ResidentGrepCorpusStorage::Mapped(mapping),
        owner_byte_ranges,
        corpus_byte_count,
    })
}

pub fn owner_for_corpus_line(
    spans: &[ResidentGrepOwnerSpan],
    line: u64,
) -> Option<&ResidentGrepOwnerSpan> {
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
        return Err("resident GREP corpus owner path is not normalized and relative".to_owned());
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/resident_grep_corpus.rs"]
mod tests;

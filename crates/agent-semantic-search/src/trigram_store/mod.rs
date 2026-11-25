// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Compact trigram posting store with 6-byte postings (`file_id`, `loc_mask`,
//! `next_mask`), flat contiguous arena + sorted lookup table, zero-copy `mmap`
//! snapshot backing, and incremental delta overlay.

pub mod snapshot;
mod store;

pub use snapshot::{
    LOOKUP_RECORD_BYTES, POSTING_RECORD_BYTES, SNAPSHOT_HEADER_BYTES, SNAPSHOT_MAGIC,
    SNAPSHOT_VERSION, SnapshotFileRecord, TrigramSnapshotError,
};
pub(crate) use store::{FlatTrigramStore, TrigramBacking};
pub use store::{
    TrigramLookupEntry, TrigramPosting, extract_file_trigram_masks, next_byte_bit, pack_trigram,
};

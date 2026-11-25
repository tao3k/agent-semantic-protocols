// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Compact `microsoft/tgrep`-inspired trigram posting store with 6-byte postings
//! (`file_id + loc_mask + next_mask`), flat contiguous arena + sorted lookup table,
//! zero-copy `mmap` snapshot backing, and incremental delta overlay.

use std::cmp::Ordering;
use std::path::Path;

use memmap2::Mmap;
use rustc_hash::FxHashMap;

use crate::trigram_query::TrigramQuery;

use super::snapshot::{
    self, LOOKUP_RECORD_BYTES, POSTING_RECORD_BYTES, SnapshotFileRecord, TrigramSnapshotError,
};

/// One compact 6-byte posting record modeled after `microsoft/tgrep`'s `index.bin`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrigramPosting {
    pub file_id: u32,
    /// Bit `(pos & 7)` is set whenever this trigram occurs at byte offset `pos`.
    pub loc_mask: u8,
    /// 8-bit Bloom mask of the byte immediately following this trigram (`pos + 3`).
    pub next_mask: u8,
}

impl TrigramPosting {
    /// Encode one posting into its 6-byte little-endian wire layout.
    #[inline]
    #[must_use]
    pub fn encode(self) -> [u8; POSTING_RECORD_BYTES] {
        let id = self.file_id.to_le_bytes();
        [id[0], id[1], id[2], id[3], self.loc_mask, self.next_mask]
    }

    /// Decode one posting from a 6-byte little-endian slice.
    #[inline]
    #[must_use]
    pub fn decode(bytes: &[u8]) -> Self {
        let file_id = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        Self {
            file_id,
            loc_mask: bytes[4],
            next_mask: bytes[5],
        }
    }
}

/// One 16-byte sorted directory entry modeled after `microsoft/tgrep`'s `lookup.bin`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrigramLookupEntry {
    pub trigram: u32,
    pub offset: u64,
    pub len: u32,
}

impl TrigramLookupEntry {
    /// Encode one lookup entry into its 16-byte little-endian wire layout.
    #[inline]
    #[must_use]
    pub fn encode(self) -> [u8; LOOKUP_RECORD_BYTES] {
        let mut out = [0u8; LOOKUP_RECORD_BYTES];
        out[0..4].copy_from_slice(&self.trigram.to_le_bytes());
        out[4..12].copy_from_slice(&self.offset.to_le_bytes());
        out[12..16].copy_from_slice(&self.len.to_le_bytes());
        out
    }

    /// Decode one lookup entry from a 16-byte little-endian slice.
    #[inline]
    #[must_use]
    pub fn decode(bytes: &[u8]) -> Self {
        let trigram = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let offset = u64::from_le_bytes([
            bytes[4], bytes[5], bytes[6], bytes[7], bytes[8], bytes[9], bytes[10], bytes[11],
        ]);
        let len = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
        Self {
            trigram,
            offset,
            len,
        }
    }
}

/// Backing storage for the base immutable flat trigram table: either owned RAM
/// vectors or a zero-copy read-only `memmap2::Mmap`.
pub(crate) enum TrigramBacking {
    Owned {
        lookup: Vec<TrigramLookupEntry>,
        postings: Vec<TrigramPosting>,
    },
    Mapped {
        mmap: Mmap,
        lookup_offset: usize,
        lookup_count: usize,
        postings_offset: usize,
        postings_count: usize,
    },
}

/// Hybrid flat trigram store: immutable `Owned` or zero-copy `Mmap` base plus
/// a small sorted in-RAM delta overlay for sub-millisecond incremental updates.
pub(crate) struct FlatTrigramStore {
    pub(crate) base: TrigramBacking,
    pub(crate) delta: FxHashMap<u32, Vec<TrigramPosting>>,
}

enum BasePostingSlice<'a> {
    Owned(&'a [TrigramPosting]),
    Mapped(&'a [u8]),
}

impl BasePostingSlice<'_> {
    #[inline]
    fn len(&self) -> usize {
        match self {
            Self::Owned(slice) => slice.len(),
            Self::Mapped(bytes) => bytes.len() / POSTING_RECORD_BYTES,
        }
    }

    #[inline]
    fn get(&self, index: usize) -> TrigramPosting {
        match self {
            Self::Owned(slice) => slice[index],
            Self::Mapped(bytes) => {
                let start = index * POSTING_RECORD_BYTES;
                TrigramPosting::decode(&bytes[start..start + POSTING_RECORD_BYTES])
            }
        }
    }

    fn binary_search_file_id(&self, target: u32) -> Option<TrigramPosting> {
        let mut lo = 0usize;
        let mut hi = self.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let posting = self.get(mid);
            match posting.file_id.cmp(&target) {
                Ordering::Equal => return Some(posting),
                Ordering::Less => lo = mid + 1,
                Ordering::Greater => hi = mid,
            }
        }
        None
    }
}

struct PostingListView<'a> {
    base: BasePostingSlice<'a>,
    delta: &'a [TrigramPosting],
}

impl PostingListView<'_> {
    #[inline]
    fn total_len(&self) -> usize {
        self.base.len() + self.delta.len()
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.total_len() == 0
    }

    fn for_each_live<F>(&self, tombstones: &[bool], mut f: F)
    where
        F: FnMut(TrigramPosting),
    {
        for i in 0..self.base.len() {
            let p = self.base.get(i);
            if !is_tombstoned(tombstones, p.file_id) {
                f(p);
            }
        }
        for &p in self.delta {
            if !is_tombstoned(tombstones, p.file_id) {
                f(p);
            }
        }
    }

    fn find_file_id(&self, file_id: u32) -> Option<TrigramPosting> {
        if let Ok(idx) = self.delta.binary_search_by_key(&file_id, |p| p.file_id) {
            return Some(self.delta[idx]);
        }
        self.base.binary_search_file_id(file_id)
    }
}

#[inline]
fn is_tombstoned(tombstones: &[bool], file_id: u32) -> bool {
    tombstones.get(file_id as usize).copied().unwrap_or(true)
}

/// Pack 3 consecutive bytes into a collision-free 24-bit trigram integer.
#[inline]
#[must_use]
pub fn pack_trigram(b0: u8, b1: u8, b2: u8) -> u32 {
    (u32::from(b0) << 16) | (u32::from(b1) << 8) | u32::from(b2)
}

/// Fast 1-cycle 8-bit Bloom hash for the byte immediately following a trigram.
#[inline]
#[must_use]
pub fn next_byte_bit(byte: u8) -> u8 {
    let bucket = (byte.wrapping_mul(31) ^ (byte >> 3)) & 7;
    1u8 << bucket
}

/// Extract all unique `(trigram, loc_mask, next_mask)` entries for a single file
/// over both `path_lower` and `body_lower`.
#[must_use]
pub fn extract_file_trigram_masks(path_lower: &[u8], body_lower: &[u8]) -> Vec<(u32, u8, u8)> {
    let cap = path_lower
        .len()
        .saturating_add(body_lower.len().min(32_768))
        .min(16_384);
    let mut map: FxHashMap<u32, (u8, u8)> =
        FxHashMap::with_capacity_and_hasher(cap, Default::default());
    accumulate_slice_trigrams(path_lower, &mut map);
    accumulate_slice_trigrams(body_lower, &mut map);
    let mut out: Vec<(u32, u8, u8)> = map
        .into_iter()
        .map(|(tri, (loc, next))| (tri, loc, next))
        .collect();
    out.sort_unstable_by_key(|entry| entry.0);
    out
}

fn accumulate_slice_trigrams(bytes: &[u8], out: &mut FxHashMap<u32, (u8, u8)>) {
    if bytes.len() < 3 {
        return;
    }
    let last_tri = bytes.len() - 3;
    for i in 0..=last_tri {
        let tri = pack_trigram(bytes[i], bytes[i + 1], bytes[i + 2]);
        let loc_bit = 1u8 << (i & 7);
        let next_bit = if i + 3 < bytes.len() {
            next_byte_bit(bytes[i + 3])
        } else {
            0
        };
        let entry = out.entry(tri).or_insert((0, 0));
        entry.0 |= loc_bit;
        entry.1 |= next_bit;
    }
}

struct LiteralTrigramConstraint {
    trigram: u32,
    rotations: u8,
    required_next_mask: u8,
}

impl LiteralTrigramConstraint {
    #[inline]
    fn allowed_start_mask(&self, posting: TrigramPosting) -> u8 {
        if (posting.next_mask & self.required_next_mask) != self.required_next_mask {
            return 0;
        }
        let mut allowed = 0xFFu8;
        let mut rots = self.rotations;
        while rots != 0 {
            let r = rots.trailing_zeros();
            allowed &= posting.loc_mask.rotate_right(r);
            if allowed == 0 {
                return 0;
            }
            rots &= rots - 1;
        }
        allowed
    }
}

fn build_literal_constraints(needle_lower: &[u8]) -> Vec<LiteralTrigramConstraint> {
    let mut constraints: Vec<LiteralTrigramConstraint> =
        Vec::with_capacity(needle_lower.len().saturating_sub(2));
    for k in 0..=needle_lower.len() - 3 {
        let tri = pack_trigram(needle_lower[k], needle_lower[k + 1], needle_lower[k + 2]);
        let rot_bit = 1u8 << (k & 7);
        let next_bit = if k + 3 < needle_lower.len() {
            next_byte_bit(needle_lower[k + 3])
        } else {
            0
        };
        if let Some(existing) = constraints.iter_mut().find(|c| c.trigram == tri) {
            existing.rotations |= rot_bit;
            existing.required_next_mask |= next_bit;
        } else {
            constraints.push(LiteralTrigramConstraint {
                trigram: tri,
                rotations: rot_bit,
                required_next_mask: next_bit,
            });
        }
    }
    constraints
}

impl FlatTrigramStore {
    #[must_use]
    pub fn build_from_file_trigrams(file_trigrams: &[Vec<(u32, u8, u8)>]) -> Self {
        let mut counts: FxHashMap<u32, u32> =
            FxHashMap::with_capacity_and_hasher(16_384, Default::default());
        let mut total_postings = 0usize;
        for list in file_trigrams {
            total_postings += list.len();
            for &(tri, _, _) in list {
                *counts.entry(tri).or_insert(0) += 1;
            }
        }

        let mut lookup: Vec<TrigramLookupEntry> = counts
            .into_iter()
            .map(|(trigram, len)| TrigramLookupEntry {
                trigram,
                offset: 0,
                len,
            })
            .collect();
        lookup.sort_unstable_by_key(|e| e.trigram);

        let mut cursor = 0u64;
        let mut tri_to_cursor: FxHashMap<u32, usize> =
            FxHashMap::with_capacity_and_hasher(lookup.len(), Default::default());
        for entry in &mut lookup {
            entry.offset = cursor;
            tri_to_cursor.insert(entry.trigram, cursor as usize);
            cursor += u64::from(entry.len);
        }

        let mut postings = vec![
            TrigramPosting {
                file_id: 0,
                loc_mask: 0,
                next_mask: 0,
            };
            total_postings
        ];

        for (idx, list) in file_trigrams.iter().enumerate() {
            let file_id = u32::try_from(idx).unwrap_or(u32::MAX);
            for &(tri, loc_mask, next_mask) in list {
                if let Some(pos) = tri_to_cursor.get_mut(&tri) {
                    postings[*pos] = TrigramPosting {
                        file_id,
                        loc_mask,
                        next_mask,
                    };
                    *pos += 1;
                }
            }
        }

        Self {
            base: TrigramBacking::Owned { lookup, postings },
            delta: FxHashMap::default(),
        }
    }

    pub fn insert_delta_file(&mut self, file_id: u32, trigrams: &[(u32, u8, u8)]) {
        for &(tri, loc_mask, next_mask) in trigrams {
            self.delta.entry(tri).or_default().push(TrigramPosting {
                file_id,
                loc_mask,
                next_mask,
            });
        }
    }

    #[must_use]
    pub fn unique_trigram_count(&self) -> usize {
        match &self.base {
            TrigramBacking::Owned { lookup, .. } => lookup.len(),
            TrigramBacking::Mapped { lookup_count, .. } => *lookup_count,
        }
    }

    #[must_use]
    pub fn base_posting_count(&self) -> usize {
        match &self.base {
            TrigramBacking::Owned { postings, .. } => postings.len(),
            TrigramBacking::Mapped { postings_count, .. } => *postings_count,
        }
    }

    #[must_use]
    pub fn is_mmap_backed(&self) -> bool {
        matches!(self.base, TrigramBacking::Mapped { .. })
    }

    #[must_use]
    pub fn encode_snapshot_bytes(&self, live_files: &[SnapshotFileRecord]) -> Vec<u8> {
        snapshot::encode_snapshot_bytes(self, live_files)
    }

    pub fn save_snapshot(
        &self,
        path: &Path,
        live_files: &[SnapshotFileRecord],
    ) -> Result<(), TrigramSnapshotError> {
        snapshot::save_snapshot(self, path, live_files)
    }

    pub fn load_mmap_snapshot(
        path: &Path,
    ) -> Result<(Vec<SnapshotFileRecord>, Self), TrigramSnapshotError> {
        snapshot::load_mmap_snapshot(path)
    }

    pub fn decode_snapshot_bytes(
        bytes: &[u8],
    ) -> Result<(Vec<SnapshotFileRecord>, Self), TrigramSnapshotError> {
        snapshot::decode_snapshot_bytes(bytes)
    }

    fn lookup_base(&self, trigram: u32) -> BasePostingSlice<'_> {
        match &self.base {
            TrigramBacking::Owned { lookup, postings } => {
                match lookup.binary_search_by_key(&trigram, |e| e.trigram) {
                    Ok(idx) => {
                        let entry = lookup[idx];
                        let start = entry.offset as usize;
                        let end = start + entry.len as usize;
                        BasePostingSlice::Owned(&postings[start..end])
                    }
                    Err(_) => BasePostingSlice::Owned(&[]),
                }
            }
            TrigramBacking::Mapped {
                mmap,
                lookup_offset,
                lookup_count,
                postings_offset,
                ..
            } => {
                let mut lo = 0usize;
                let mut hi = *lookup_count;
                while lo < hi {
                    let mid = lo + (hi - lo) / 2;
                    let rec_off = *lookup_offset + mid * LOOKUP_RECORD_BYTES;
                    let entry =
                        TrigramLookupEntry::decode(&mmap[rec_off..rec_off + LOOKUP_RECORD_BYTES]);
                    match entry.trigram.cmp(&trigram) {
                        Ordering::Equal => {
                            let start =
                                *postings_offset + (entry.offset as usize) * POSTING_RECORD_BYTES;
                            let byte_len = (entry.len as usize) * POSTING_RECORD_BYTES;
                            return BasePostingSlice::Mapped(&mmap[start..start + byte_len]);
                        }
                        Ordering::Less => lo = mid + 1,
                        Ordering::Greater => hi = mid,
                    }
                }
                BasePostingSlice::Mapped(&[])
            }
        }
    }

    fn lookup_view(&self, trigram: u32) -> PostingListView<'_> {
        let base = self.lookup_base(trigram);
        let delta = self.delta.get(&trigram).map_or(&[][..], Vec::as_slice);
        PostingListView { base, delta }
    }

    #[must_use]
    pub fn query_literal(&self, needle_lower: &[u8], tombstones: &[bool]) -> Vec<u32> {
        if needle_lower.len() < 3 {
            return all_live_file_ids(tombstones);
        }
        let constraints = build_literal_constraints(needle_lower);
        let mut resolved: Vec<(LiteralTrigramConstraint, PostingListView<'_>)> =
            Vec::with_capacity(constraints.len());
        for constraint in constraints {
            let view = self.lookup_view(constraint.trigram);
            if view.is_empty() {
                return Vec::new();
            }
            resolved.push((constraint, view));
        }

        resolved.sort_unstable_by_key(|(_, view)| view.total_len());

        let (first_constraint, first_view) = &resolved[0];
        let mut candidates: Vec<(u32, u8)> = Vec::with_capacity(first_view.total_len().min(4096));
        first_view.for_each_live(tombstones, |posting| {
            let start_mask = first_constraint.allowed_start_mask(posting);
            if start_mask != 0 {
                candidates.push((posting.file_id, start_mask));
            }
        });

        for (constraint, view) in &resolved[1..] {
            if candidates.is_empty() {
                break;
            }
            let mut next_candidates = Vec::with_capacity(candidates.len());
            if candidates.len().saturating_mul(8) < view.total_len() {
                for &(file_id, prev_mask) in &candidates {
                    if let Some(posting) = view.find_file_id(file_id) {
                        let combined = prev_mask & constraint.allowed_start_mask(posting);
                        if combined != 0 {
                            next_candidates.push((file_id, combined));
                        }
                    }
                }
            } else {
                merge_candidates_with_view(&candidates, view, constraint, &mut next_candidates);
            }
            candidates = next_candidates;
        }

        candidates.into_iter().map(|(id, _)| id).collect()
    }

    #[must_use]
    pub fn evaluate_query(&self, query: &TrigramQuery, tombstones: &[bool]) -> Vec<u32> {
        match query {
            TrigramQuery::All => all_live_file_ids(tombstones),
            TrigramQuery::None => Vec::new(),
            TrigramQuery::Literal(lit) => self.query_literal(lit, tombstones),
            TrigramQuery::And(children) => {
                if children.is_empty() {
                    return all_live_file_ids(tombstones);
                }
                let mut sets: Vec<Vec<u32>> = Vec::with_capacity(children.len());
                for child in children {
                    let s = self.evaluate_query(child, tombstones);
                    if s.is_empty() {
                        return Vec::new();
                    }
                    sets.push(s);
                }
                sets.sort_unstable_by_key(Vec::len);
                let mut acc = sets.swap_remove(0);
                for other in &sets {
                    acc = intersect_sorted_ids(&acc, other);
                    if acc.is_empty() {
                        break;
                    }
                }
                acc
            }
            TrigramQuery::Or(children) => {
                let mut out: Vec<u32> = Vec::new();
                for child in children {
                    if matches!(child, TrigramQuery::All) {
                        return all_live_file_ids(tombstones);
                    }
                    let branch = self.evaluate_query(child, tombstones);
                    out = union_sorted_ids(&out, &branch);
                }
                out
            }
        }
    }
}

fn merge_candidates_with_view(
    candidates: &[(u32, u8)],
    view: &PostingListView<'_>,
    constraint: &LiteralTrigramConstraint,
    out: &mut Vec<(u32, u8)>,
) {
    let mut ci = 0usize;
    let mut bi = 0usize;
    let base_len = view.base.len();
    while ci < candidates.len() && bi < base_len {
        let (c_id, c_mask) = candidates[ci];
        let p = view.base.get(bi);
        match c_id.cmp(&p.file_id) {
            Ordering::Equal => {
                let combined = c_mask & constraint.allowed_start_mask(p);
                if combined != 0 {
                    out.push((c_id, combined));
                }
                ci += 1;
                bi += 1;
            }
            Ordering::Less => ci += 1,
            Ordering::Greater => bi += 1,
        }
    }

    let mut di = 0usize;
    let delta = view.delta;
    while ci < candidates.len() && di < delta.len() {
        let (c_id, c_mask) = candidates[ci];
        let p = delta[di];
        match c_id.cmp(&p.file_id) {
            Ordering::Equal => {
                let combined = c_mask & constraint.allowed_start_mask(p);
                if combined != 0 {
                    out.push((c_id, combined));
                }
                ci += 1;
                di += 1;
            }
            Ordering::Less => ci += 1,
            Ordering::Greater => di += 1,
        }
    }
}

fn all_live_file_ids(tombstones: &[bool]) -> Vec<u32> {
    tombstones
        .iter()
        .enumerate()
        .filter_map(|(idx, &dead)| (!dead).then(|| u32::try_from(idx).ok()).flatten())
        .collect()
}

pub(crate) fn intersect_sorted_ids(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut out = Vec::with_capacity(a.len().min(b.len()));
    if a.len().saturating_mul(8) < b.len() {
        for &val in a {
            if b.binary_search(&val).is_ok() {
                out.push(val);
            }
        }
        return out;
    }
    let (mut i, mut j) = (0usize, 0usize);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            Ordering::Equal => {
                out.push(a[i]);
                i += 1;
                j += 1;
            }
            Ordering::Less => i += 1,
            Ordering::Greater => j += 1,
        }
    }
    out
}

pub(crate) fn union_sorted_ids(a: &[u32], b: &[u32]) -> Vec<u32> {
    if a.is_empty() {
        return b.to_vec();
    }
    if b.is_empty() {
        return a.to_vec();
    }
    let mut out = Vec::with_capacity(a.len() + b.len());
    let (mut i, mut j) = (0usize, 0usize);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            Ordering::Equal => {
                out.push(a[i]);
                i += 1;
                j += 1;
            }
            Ordering::Less => {
                out.push(a[i]);
                i += 1;
            }
            Ordering::Greater => {
                out.push(b[j]);
                j += 1;
            }
        }
    }
    out.extend_from_slice(&a[i..]);
    out.extend_from_slice(&b[j..]);
    out
}

// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use rustc_hash::{FxHashMap, FxHashSet};

use crate::RESIDENT_BYTE_GRAM_WIDTH;

const MAGIC: &[u8; 16] = b"ASPBYTECOVER1\0\0\0";
const VERSION: u32 = 1;
const HEADER_LEN: usize = 96;
const DIRECTORY_ENTRY_LEN: usize = 32;

#[derive(Clone, Copy, Debug)]
struct DirectoryEntry {
    gram: u32,
    posting_offset: usize,
    posting_len: usize,
    posting_count: usize,
}

pub(super) struct EncodedByteCoverageArtifact {
    pub(super) bytes: Vec<u8>,
    pub(super) layout: ValidatedByteCoverageLayout,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ValidatedByteCoverageLayout {
    owner_count: usize,
    gram_count: usize,
    posting_count: usize,
    postings_offset: usize,
}

pub(super) fn pack_gram(first: u8, second: u8, third: u8) -> u32 {
    (u32::from(first) << 16) | (u32::from(second) << 8) | u32::from(third)
}

#[cfg(test)]
pub(super) fn encode<'a>(owners: impl IntoIterator<Item = &'a [u8]>) -> Result<Vec<u8>, String> {
    encode_validated(owners).map(|encoded| encoded.bytes)
}

pub(super) fn encode_validated<'a>(
    owners: impl IntoIterator<Item = &'a [u8]>,
) -> Result<EncodedByteCoverageArtifact, String> {
    let owners = owners.into_iter().collect::<Vec<_>>();
    let owner_count = owners.len();
    if owner_count > u32::MAX as usize {
        return Err("resident byte predicate owner count exceeds u32".to_owned());
    }
    // Fx hashing is sufficient for packed fixed-width integer grams and avoids
    // paying SipHash or ordered-tree insertion cost for every owner/gram pair.
    // The final key sort keeps the V1 artifact byte-for-byte deterministic.
    let mut postings = FxHashMap::<u32, Vec<u32>>::default();
    let mut owner_grams = FxHashSet::default();
    for (owner_id, bytes) in owners.into_iter().enumerate() {
        owner_grams.clear();
        owner_grams.reserve(bytes.len().min(65_536));
        for window in bytes.windows(RESIDENT_BYTE_GRAM_WIDTH) {
            owner_grams.insert(pack_gram(window[0], window[1], window[2]));
        }
        let owner_id = u32::try_from(owner_id)
            .map_err(|_| "resident byte predicate owner id exceeds u32".to_owned())?;
        for &gram in &owner_grams {
            postings.entry(gram).or_default().push(owner_id);
        }
    }
    let mut postings = postings.into_iter().collect::<Vec<_>>();
    postings.sort_unstable_by_key(|(gram, _)| *gram);
    let gram_count = postings.len();
    let posting_count = postings.iter().try_fold(0usize, |total, (_, posting)| {
        total
            .checked_add(posting.len())
            .ok_or_else(|| "resident byte predicate posting count overflows".to_owned())
    })?;
    let directory_len = gram_count
        .checked_mul(DIRECTORY_ENTRY_LEN)
        .ok_or_else(|| "resident byte predicate directory length overflows".to_owned())?;
    let postings_offset = HEADER_LEN
        .checked_add(directory_len)
        .ok_or_else(|| "resident byte predicate postings offset overflows".to_owned())?;
    let mut directory = Vec::with_capacity(directory_len);
    let mut posting_bytes = Vec::new();
    for (gram, owner_ids) in postings {
        let relative_offset = posting_bytes.len();
        encode_posting(&owner_ids, &mut posting_bytes);
        write_u32(&mut directory, gram);
        write_u32(&mut directory, 0);
        write_u64(
            &mut directory,
            checked_u64(relative_offset, "posting offset")?,
        );
        write_u64(
            &mut directory,
            checked_u64(posting_bytes.len() - relative_offset, "posting length")?,
        );
        write_u64(
            &mut directory,
            checked_u64(owner_ids.len(), "posting count")?,
        );
    }
    let total_len = postings_offset
        .checked_add(posting_bytes.len())
        .ok_or_else(|| "resident byte predicate total length overflows".to_owned())?;
    let mut payload_hasher = blake3::Hasher::new();
    payload_hasher.update(&directory);
    payload_hasher.update(&posting_bytes);
    let mut encoded = Vec::with_capacity(total_len);
    encoded.extend_from_slice(MAGIC);
    write_u32(&mut encoded, VERSION);
    write_u32(&mut encoded, RESIDENT_BYTE_GRAM_WIDTH as u32);
    write_u64(&mut encoded, checked_u64(owner_count, "owner count")?);
    write_u64(&mut encoded, checked_u64(gram_count, "gram count")?);
    write_u64(&mut encoded, checked_u64(posting_count, "posting count")?);
    write_u64(
        &mut encoded,
        checked_u64(postings_offset, "postings offset")?,
    );
    write_u64(&mut encoded, checked_u64(total_len, "total length")?);
    encoded.extend_from_slice(payload_hasher.finalize().as_bytes());
    encoded.extend_from_slice(&directory);
    encoded.extend_from_slice(&posting_bytes);
    Ok(EncodedByteCoverageArtifact {
        bytes: encoded,
        layout: ValidatedByteCoverageLayout {
            owner_count,
            gram_count,
            posting_count,
            postings_offset,
        },
    })
}

impl ValidatedByteCoverageLayout {
    pub(super) fn parse(bytes: &[u8], expected_owner_count: usize) -> Result<Self, String> {
        if bytes.len() < HEADER_LEN {
            return Err(
                "resident byte predicate artifact is truncated before its header".to_owned(),
            );
        }
        if &bytes[..MAGIC.len()] != MAGIC {
            return Err("resident byte predicate artifact magic is invalid".to_owned());
        }
        if read_u32(bytes, 16)? != VERSION {
            return Err("resident byte predicate artifact version is unsupported".to_owned());
        }
        if read_u32(bytes, 20)? as usize != RESIDENT_BYTE_GRAM_WIDTH {
            return Err("resident byte predicate gram width is invalid".to_owned());
        }
        let owner_count = checked_usize(read_u64(bytes, 24)?, "owner count")?;
        if owner_count > u32::MAX as usize || owner_count != expected_owner_count {
            return Err("resident byte predicate owner count mismatch".to_owned());
        }
        let gram_count = checked_usize(read_u64(bytes, 32)?, "gram count")?;
        let posting_count = checked_usize(read_u64(bytes, 40)?, "posting count")?;
        let postings_offset = checked_usize(read_u64(bytes, 48)?, "postings offset")?;
        let total_len = checked_usize(read_u64(bytes, 56)?, "total length")?;
        if total_len != bytes.len() {
            return Err("resident byte predicate total length mismatch".to_owned());
        }
        let expected_postings_offset =
            HEADER_LEN
                .checked_add(gram_count.checked_mul(DIRECTORY_ENTRY_LEN).ok_or_else(|| {
                    "resident byte predicate directory length overflows".to_owned()
                })?)
                .ok_or_else(|| "resident byte predicate postings offset overflows".to_owned())?;
        if postings_offset != expected_postings_offset || postings_offset > bytes.len() {
            return Err("resident byte predicate directory range is invalid".to_owned());
        }
        let expected_digest = bytes
            .get(64..96)
            .ok_or_else(|| "resident byte predicate payload digest is truncated".to_owned())?;
        if blake3::hash(&bytes[HEADER_LEN..]).as_bytes() != expected_digest {
            return Err("resident byte predicate payload digest mismatch".to_owned());
        }
        let layout = Self {
            owner_count,
            gram_count,
            posting_count,
            postings_offset,
        };
        layout.validate_directory(bytes)?;
        Ok(layout)
    }

    pub(super) fn gram_count(self) -> usize {
        self.gram_count
    }

    pub(super) fn posting_count(self) -> usize {
        self.posting_count
    }

    pub(super) fn posting(self, bytes: &[u8], gram: u32) -> Result<Option<Vec<u32>>, String> {
        self.find_entry(bytes, gram)?
            .map(|entry| self.decode_posting(bytes, entry))
            .transpose()
    }

    pub(super) fn lookup_posting_count(
        self,
        bytes: &[u8],
        gram: u32,
    ) -> Result<Option<usize>, String> {
        Ok(self
            .find_entry(bytes, gram)?
            .map(|entry| entry.posting_count))
    }

    fn find_entry(self, bytes: &[u8], gram: u32) -> Result<Option<DirectoryEntry>, String> {
        let mut lower = 0usize;
        let mut upper = self.gram_count;
        while lower < upper {
            let middle = lower + (upper - lower) / 2;
            let entry = self.entry(bytes, middle)?;
            match entry.gram.cmp(&gram) {
                std::cmp::Ordering::Less => lower = middle + 1,
                std::cmp::Ordering::Greater => upper = middle,
                std::cmp::Ordering::Equal => return Ok(Some(entry)),
            }
        }
        Ok(None)
    }

    fn validate_directory(self, bytes: &[u8]) -> Result<(), String> {
        let mut previous_gram = None;
        let mut previous_end = 0usize;
        let mut actual_posting_count = 0usize;
        for index in 0..self.gram_count {
            let entry = self.entry(bytes, index)?;
            if previous_gram.is_some_and(|previous| previous >= entry.gram) {
                return Err("resident byte predicate grams are not strictly sorted".to_owned());
            }
            if entry.posting_offset != previous_end {
                return Err("resident byte predicate posting ranges are not canonical".to_owned());
            }
            let mut decoder = self.decoder(bytes, entry)?;
            while decoder.next_owner()?.is_some() {}
            actual_posting_count = actual_posting_count
                .checked_add(decoder.decoded)
                .ok_or_else(|| "resident byte predicate posting count overflows".to_owned())?;
            previous_end = entry
                .posting_offset
                .checked_add(entry.posting_len)
                .ok_or_else(|| "resident byte predicate posting range overflows".to_owned())?;
            previous_gram = Some(entry.gram);
        }
        let terminal = self
            .postings_offset
            .checked_add(previous_end)
            .ok_or_else(|| "resident byte predicate terminal range overflows".to_owned())?;
        if terminal != bytes.len() || actual_posting_count != self.posting_count {
            return Err("resident byte predicate posting coverage mismatch".to_owned());
        }
        Ok(())
    }

    fn entry(self, bytes: &[u8], index: usize) -> Result<DirectoryEntry, String> {
        let start = HEADER_LEN
            .checked_add(
                index
                    .checked_mul(DIRECTORY_ENTRY_LEN)
                    .ok_or_else(|| "resident byte predicate entry offset overflows".to_owned())?,
            )
            .ok_or_else(|| "resident byte predicate entry offset overflows".to_owned())?;
        let end = start
            .checked_add(DIRECTORY_ENTRY_LEN)
            .ok_or_else(|| "resident byte predicate entry range overflows".to_owned())?;
        let entry = bytes
            .get(start..end)
            .ok_or_else(|| "resident byte predicate directory entry is truncated".to_owned())?;
        if read_u32(entry, 4)? != 0 {
            return Err("resident byte predicate directory reserved bytes are nonzero".to_owned());
        }
        Ok(DirectoryEntry {
            gram: read_u32(entry, 0)?,
            posting_offset: checked_usize(read_u64(entry, 8)?, "posting offset")?,
            posting_len: checked_usize(read_u64(entry, 16)?, "posting length")?,
            posting_count: checked_usize(read_u64(entry, 24)?, "posting count")?,
        })
    }

    fn decode_posting(self, bytes: &[u8], entry: DirectoryEntry) -> Result<Vec<u32>, String> {
        let mut decoder = self.decoder(bytes, entry)?;
        let mut owners = Vec::with_capacity(entry.posting_count);
        while let Some(owner) = decoder.next_owner()? {
            owners.push(owner);
        }
        Ok(owners)
    }

    pub(super) fn intersect_posting(
        self,
        bytes: &[u8],
        gram: u32,
        candidates: &mut Vec<u32>,
    ) -> Result<usize, String> {
        let entry = self
            .find_entry(bytes, gram)?
            .ok_or_else(|| "resident GREP posting disappeared after metadata lookup".to_owned())?;
        let mut decoder = self.decoder(bytes, entry)?;
        let mut read = 0;
        let mut written = 0;
        while read < candidates.len() {
            let Some(owner) = decoder.next_owner()? else {
                break;
            };
            while read < candidates.len() && candidates[read] < owner {
                read += 1;
            }
            if read < candidates.len() && candidates[read] == owner {
                candidates[written] = owner;
                written += 1;
                read += 1;
            }
        }
        candidates.truncate(written);
        Ok(decoder.decoded)
    }

    fn decoder<'a>(
        self,
        bytes: &'a [u8],
        entry: DirectoryEntry,
    ) -> Result<PostingDecoder<'a>, String> {
        let start = self
            .postings_offset
            .checked_add(entry.posting_offset)
            .ok_or_else(|| "resident byte predicate posting start overflows".to_owned())?;
        let end = start
            .checked_add(entry.posting_len)
            .ok_or_else(|| "resident byte predicate posting end overflows".to_owned())?;
        let encoded = bytes
            .get(start..end)
            .ok_or_else(|| "resident byte predicate posting exceeds artifact bounds".to_owned())?;
        if entry.posting_count == 0
            || entry.posting_count > encoded.len()
            || entry.posting_count > self.owner_count
        {
            return Err("resident byte predicate posting count exceeds bounds".to_owned());
        }
        Ok(PostingDecoder {
            encoded,
            position: 0,
            previous: 0,
            decoded: 0,
            expected: entry.posting_count,
            owner_count: self.owner_count,
        })
    }
}

struct PostingDecoder<'a> {
    encoded: &'a [u8],
    position: usize,
    previous: u32,
    decoded: usize,
    expected: usize,
    owner_count: usize,
}

impl PostingDecoder<'_> {
    fn next_owner(&mut self) -> Result<Option<u32>, String> {
        if self.position == self.encoded.len() {
            return if self.decoded == self.expected {
                Ok(None)
            } else {
                Err("resident byte predicate posting count mismatch".to_owned())
            };
        }
        if self.decoded >= self.expected {
            return Err("resident byte predicate posting count mismatch".to_owned());
        }
        let delta = read_varint(self.encoded, &mut self.position)?;
        let owner = self
            .previous
            .checked_add(delta)
            .ok_or_else(|| "resident byte predicate owner delta overflows".to_owned())?;
        if owner as usize >= self.owner_count || (self.decoded != 0 && delta == 0) {
            return Err("resident byte predicate posting owner order is invalid".to_owned());
        }
        self.previous = owner;
        self.decoded += 1;
        Ok(Some(owner))
    }
}

fn encode_posting(owners: &[u32], output: &mut Vec<u8>) {
    let mut previous = 0u32;
    for (index, owner) in owners.iter().copied().enumerate() {
        let delta = if index == 0 { owner } else { owner - previous };
        write_varint(output, delta);
        previous = owner;
    }
}

fn write_varint(output: &mut Vec<u8>, mut value: u32) {
    while value >= 0x80 {
        output.push((value as u8 & 0x7f) | 0x80);
        value >>= 7;
    }
    output.push(value as u8);
}

fn read_varint(bytes: &[u8], position: &mut usize) -> Result<u32, String> {
    let mut value = 0u32;
    for shift in (0..=28).step_by(7) {
        let byte = *bytes
            .get(*position)
            .ok_or_else(|| "resident byte predicate varint is truncated".to_owned())?;
        *position += 1;
        if shift == 28 && byte > 0x0f {
            return Err("resident byte predicate varint overflows u32".to_owned());
        }
        value |= u32::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err("resident byte predicate varint is overlong".to_owned())
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| "resident byte predicate u32 offset overflows".to_owned())?;
    let field = bytes
        .get(offset..end)
        .ok_or_else(|| "resident byte predicate u32 field is truncated".to_owned())?;
    Ok(u32::from_le_bytes(field.try_into().map_err(|_| {
        "resident byte predicate u32 field width is invalid".to_owned()
    })?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, String> {
    let end = offset
        .checked_add(8)
        .ok_or_else(|| "resident byte predicate u64 offset overflows".to_owned())?;
    let field = bytes
        .get(offset..end)
        .ok_or_else(|| "resident byte predicate u64 field is truncated".to_owned())?;
    Ok(u64::from_le_bytes(field.try_into().map_err(|_| {
        "resident byte predicate u64 field width is invalid".to_owned()
    })?))
}

fn write_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn checked_u64(value: usize, field: &str) -> Result<u64, String> {
    u64::try_from(value).map_err(|_| format!("resident byte predicate {field} exceeds u64"))
}

fn checked_usize(value: u64, field: &str) -> Result<usize, String> {
    usize::try_from(value).map_err(|_| format!("resident byte predicate {field} exceeds usize"))
}

#[cfg(test)]
#[path = "../tests/unit/resident_byte_coverage_format.rs"]
mod tests;

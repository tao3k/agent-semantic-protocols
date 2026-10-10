// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{
    DIRECTORY_ENTRY_LEN, HEADER_LEN, MAGIC, RESIDENT_BYTE_GRAM_WIDTH, VERSION,
    ValidatedByteCoverageLayout, checked_u64, encode, pack_gram, write_u32, write_u64,
    write_varint,
};

#[test]
fn forged_posting_count_is_rejected_before_allocation() {
    let mut bytes = encode([b"abc".as_slice()]).unwrap();
    bytes[HEADER_LEN + 24..HEADER_LEN + 32].copy_from_slice(&u64::MAX.to_le_bytes());
    let digest = blake3::hash(&bytes[HEADER_LEN..]);
    bytes[64..96].copy_from_slice(digest.as_bytes());
    assert!(
        ValidatedByteCoverageLayout::parse(&bytes, 1)
            .unwrap_err()
            .contains("count")
    );
}

#[test]
fn streaming_intersection_matches_reference_for_all_small_subsets() {
    let bytes = encode((0..8).map(|_| b"abc".as_slice())).unwrap();
    let layout = ValidatedByteCoverageLayout::parse(&bytes, 8).unwrap();
    for mask in 0u32..256 {
        let expected = (0..8)
            .filter(|id| mask & (1 << id) != 0)
            .collect::<Vec<u32>>();
        let mut candidates = expected.clone();
        let decoded = layout
            .intersect_posting(&bytes, pack_gram(b'a', b'b', b'c'), &mut candidates)
            .unwrap();
        assert_eq!(candidates, expected);
        assert_eq!(decoded, expected.last().map_or(0, |id| *id as usize + 1));
    }
}

#[test]
fn streaming_intersection_handles_gaps_and_exhaustion() {
    let bytes = encode([b"abc".as_slice(), b"xyz", b"abc", b"xyz"]).unwrap();
    let layout = ValidatedByteCoverageLayout::parse(&bytes, 4).unwrap();
    for mask in 0u32..16 {
        let mut candidates = (0..4)
            .filter(|id| mask & (1 << id) != 0)
            .collect::<Vec<u32>>();
        let expected = candidates
            .iter()
            .copied()
            .filter(|id| *id == 0 || *id == 2)
            .collect::<Vec<_>>();
        let decoded = layout
            .intersect_posting(&bytes, pack_gram(b'a', b'b', b'c'), &mut candidates)
            .unwrap();
        assert_eq!(candidates, expected);
        assert!(decoded <= 2);
    }
}

#[test]
fn flat_arena_is_byte_identical_to_fragmented_v1_reference() {
    let owners = [
        b"abcabc alpha".as_slice(),
        b"beta abc".as_slice(),
        "\u{4f60}\u{597d}abc".as_bytes(),
        b"".as_slice(),
    ];
    assert_eq!(encode(owners).unwrap(), fragmented_v1_reference(owners));
}

/// Frozen test-only model of the pre-arena V1 encoder. Keeping this outside
/// production proves that changing construction topology does not change one
/// byte of the admitted mmap format.
fn fragmented_v1_reference(owners: [&[u8]; 4]) -> Vec<u8> {
    let mut postings = std::collections::BTreeMap::<u32, Vec<u32>>::new();
    for (owner_id, bytes) in owners.iter().enumerate() {
        let grams = bytes
            .windows(RESIDENT_BYTE_GRAM_WIDTH)
            .map(|window| pack_gram(window[0], window[1], window[2]))
            .collect::<std::collections::BTreeSet<_>>();
        for gram in grams {
            postings.entry(gram).or_default().push(owner_id as u32);
        }
    }
    let posting_count = postings.values().map(Vec::len).sum::<usize>();
    let postings_offset = HEADER_LEN + postings.len() * DIRECTORY_ENTRY_LEN;
    let mut directory = Vec::with_capacity(postings.len() * DIRECTORY_ENTRY_LEN);
    let mut posting_bytes = Vec::with_capacity(posting_count);
    for (gram, owner_ids) in postings {
        let relative_offset = posting_bytes.len();
        let mut previous = 0;
        for (index, owner) in owner_ids.iter().copied().enumerate() {
            write_varint(
                &mut posting_bytes,
                if index == 0 { owner } else { owner - previous },
            );
            previous = owner;
        }
        write_u32(&mut directory, gram);
        write_u32(&mut directory, 0);
        write_u64(
            &mut directory,
            checked_u64(relative_offset, "posting offset").unwrap(),
        );
        write_u64(
            &mut directory,
            checked_u64(posting_bytes.len() - relative_offset, "posting length").unwrap(),
        );
        write_u64(
            &mut directory,
            checked_u64(owner_ids.len(), "posting count").unwrap(),
        );
    }
    let total_len = postings_offset + posting_bytes.len();
    let mut payload_hasher = blake3::Hasher::new();
    payload_hasher.update(&directory);
    payload_hasher.update(&posting_bytes);
    let mut encoded = Vec::with_capacity(total_len);
    encoded.extend_from_slice(MAGIC);
    write_u32(&mut encoded, VERSION);
    write_u32(&mut encoded, RESIDENT_BYTE_GRAM_WIDTH as u32);
    write_u64(&mut encoded, owners.len() as u64);
    write_u64(&mut encoded, (directory.len() / DIRECTORY_ENTRY_LEN) as u64);
    write_u64(&mut encoded, posting_count as u64);
    write_u64(&mut encoded, postings_offset as u64);
    write_u64(&mut encoded, total_len as u64);
    encoded.extend_from_slice(payload_hasher.finalize().as_bytes());
    encoded.extend_from_slice(&directory);
    encoded.extend_from_slice(&posting_bytes);
    encoded
}

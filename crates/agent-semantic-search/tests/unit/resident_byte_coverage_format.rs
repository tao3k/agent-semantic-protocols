// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{HEADER_LEN, ValidatedByteCoverageLayout, encode, pack_gram};

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

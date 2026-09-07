// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use crate::ValidatedSortedRecordTable;
use crate::encode_sorted_record_table;

#[test]
fn sorted_record_table_reads_one_value_without_materializing_the_table() {
    let bytes = encode_sorted_record_table(vec![
        (b"zeta".to_vec(), b"third".to_vec()),
        (b"alpha".to_vec(), b"first".to_vec()),
        (b"middle".to_vec(), b"second".to_vec()),
    ])
    .expect("encode table");
    let table = ValidatedSortedRecordTable::parse(&bytes).expect("validate table");
    assert_eq!(table.len(), 3);
    assert_eq!(table.get(b"middle"), Some(b"second".as_slice()));
    assert_eq!(table.get(b"absent"), None);
}

#[test]
fn duplicate_truncated_or_noncanonical_tables_are_rejected() {
    assert!(
        encode_sorted_record_table(vec![
            (b"same".to_vec(), vec![1]),
            (b"same".to_vec(), vec![2]),
        ])
        .is_err()
    );

    let bytes =
        encode_sorted_record_table(vec![(b"one".to_vec(), vec![1, 2, 3])]).expect("encode table");
    assert!(ValidatedSortedRecordTable::parse(&bytes[..bytes.len() - 1]).is_err());

    let mut trailing = bytes;
    trailing.push(0);
    assert!(ValidatedSortedRecordTable::parse(&trailing).is_err());
}

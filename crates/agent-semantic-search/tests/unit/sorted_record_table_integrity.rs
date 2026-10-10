// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::ValidatedSortedRecordTable;
use super::encode_sorted_record_table;

#[test]
fn touched_record_digest_rejects_payload_corruption_without_full_table_scan() {
    let mut encoded =
        encode_sorted_record_table(vec![(b"owner-key".to_vec(), b"owner-value".to_vec())])
            .expect("encode one record");
    let last = encoded
        .last_mut()
        .expect("encoded table contains a record payload");
    *last ^= 0x01;

    let table = ValidatedSortedRecordTable::parse(&encoded)
        .expect("cold open validates only the fixed directory");
    assert_eq!(
        table.get_checked(b"owner-key").unwrap_err(),
        "sorted record table record digest mismatch"
    );
}

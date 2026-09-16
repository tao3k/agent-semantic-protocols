// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::partition_affected_owner_membership;
use std::collections::BTreeSet;

#[test]
fn replacement_membership_partitions_upserts_and_tombstones() {
    let affected = BTreeSet::from([
        "removed.rs".to_owned(),
        "replaced.rs".to_owned(),
        "unchanged-outside-provider.rs".to_owned(),
    ]);
    let present = BTreeSet::from([
        "new.rs".to_owned(),
        "replaced.rs".to_owned(),
        "unchanged-outside-provider.rs".to_owned(),
    ]);

    let (upserts, tombstones) = partition_affected_owner_membership(&affected, &present);

    assert_eq!(
        upserts,
        BTreeSet::from([
            "replaced.rs".to_owned(),
            "unchanged-outside-provider.rs".to_owned(),
        ])
    );
    assert_eq!(tombstones, BTreeSet::from(["removed.rs".to_owned()]));
    assert!(upserts.is_disjoint(&tombstones));
    assert!(!upserts.contains("new.rs"));
}

// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use super::{
    ExactProjectionKind, WorkspaceOwnerSearchSeedSnapshot, finish_owner_search,
    selector_is_admitted,
};

fn seed(selector: &str) -> WorkspaceOwnerSearchSeedSnapshot {
    WorkspaceOwnerSearchSeedSnapshot {
        selector: selector.to_owned(),
        byte_start: 0,
        byte_end: selector.len(),
    }
}

#[test]
fn empty_query_admits_source_but_never_callable_projection() {
    assert!(
        selector_is_admitted(ExactProjectionKind::Source, &[], &[]).expect("empty source query")
    );
    assert!(
        !selector_is_admitted(ExactProjectionKind::CallableSkeleton, &[], &[])
            .expect("empty callable query")
    );
}

#[test]
fn finishing_search_is_canonical_before_limit_and_rejects_duplicates() {
    let (candidate_count, selectors) =
        finish_owner_search(vec![seed("rust://z"), seed("rust://a")], 1)
            .expect("canonical owner search");
    assert_eq!(candidate_count, 2);
    assert_eq!(selectors, vec![seed("rust://a")]);
    assert!(finish_owner_search(vec![seed("rust://a"), seed("rust://a")], 8).is_err());
}

#[test]
fn noncanonical_query_keys_fail_closed_before_matching() {
    for query_keys in [
        vec!["".to_owned()],
        vec!["z".to_owned(), "a".to_owned()],
        vec!["a".to_owned(), "a".to_owned()],
    ] {
        assert!(
            selector_is_admitted(ExactProjectionKind::Source, &query_keys, &["a".to_owned()])
                .is_err()
        );
    }
}

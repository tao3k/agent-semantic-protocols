// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

#[test]
fn object_keys_are_sorted_recursively() {
    let value = serde_json::json!({"z": {"b": 2, "a": 1}, "a": [true, "x"]});
    assert_eq!(
        super::to_jcs_vec(&value).expect("canonical JSON"),
        br#"{"a":[true,"x"],"z":{"a":1,"b":2}}"#
    );
}

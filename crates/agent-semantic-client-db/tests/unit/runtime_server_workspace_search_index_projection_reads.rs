use super::validate_cold_lexical_request;

#[test]
fn progressive_acquisition_accepts_4096_candidates_but_rejects_larger_requests() {
    assert!(validate_cold_lexical_request("runtime|query", 4096).is_ok());
    assert!(validate_cold_lexical_request("runtime|query", 4097).is_ok());
    assert!(validate_cold_lexical_request("runtime|query", 0).is_err());
    assert!(validate_cold_lexical_request(" ", 30).is_err());
}
// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

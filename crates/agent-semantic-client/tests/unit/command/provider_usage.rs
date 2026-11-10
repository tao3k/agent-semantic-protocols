// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::provider_usage;

#[test]
fn removed_language_commands_are_absent_from_provider_usage() {
    let usage = provider_usage();

    assert!(!usage.contains("check"), "removed Check leaked: {usage}");
    assert!(
        !usage.contains("evidence"),
        "removed Evidence leaked: {usage}"
    );
}

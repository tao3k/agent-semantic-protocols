// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Selects the highest-priority policy candidate without encoding rule identity.

use crate::hook_config::HookPolicyCandidate;

pub(crate) fn higher_priority_candidate(
    left: Option<HookPolicyCandidate>,
    right: Option<HookPolicyCandidate>,
) -> Option<HookPolicyCandidate> {
    match (left, right) {
        (Some(left), Some(right)) => Some(if right.priority > left.priority {
            right
        } else {
            left
        }),
        (Some(candidate), None) | (None, Some(candidate)) => Some(candidate),
        (None, None) => None,
    }
}

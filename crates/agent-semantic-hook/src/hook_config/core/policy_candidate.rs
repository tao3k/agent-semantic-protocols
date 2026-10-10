// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

pub(crate) struct HookPolicyCandidate {
    pub(crate) priority: i64,
    pub(crate) terminal: bool,
    pub(crate) decision: crate::HookDecision,
}
